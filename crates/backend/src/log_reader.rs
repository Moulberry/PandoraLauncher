use std::{borrow::Cow, io::{BufRead, BufReader, PipeReader}, sync::Arc};

use bridge::{
    game_output::GameOutputLogLevel, handle::FrontendHandle, message::{GameOutputMsg, MessageToFrontend},
};
use chrono::Utc;
use memchr::memchr;
use once_cell::sync::Lazy;
use regex::Regex;
use thiserror::Error;

static REPLACEMENTS: Lazy<[(Regex, &'static str); 7]> = Lazy::new(|| {
    [
        // Access token replacements
        (regex::Regex::new(r#"SignedJWT: [^\s]+"#).unwrap(), "SignedJWT: *****"),
        (regex::Regex::new(r#"Session ID is [^\s)]+"#).unwrap(), "Session ID is *****"),
        (regex::Regex::new(r#"--accessToken, [^\s,]+"#).unwrap(), "--accessToken, *****"),
        // Computer username replacements
        (regex::Regex::new(r#"\/home\/[^/]+\/"#).unwrap(), "/home/*****/"),
        (regex::Regex::new(r#"\/Users\/[^/]+\/"#).unwrap(), "/Users/*****/"),
        (regex::Regex::new(r#"\\Users\\[^\\]+\\"#).unwrap(), "\\Users\\*****\\"),
        (regex::Regex::new(r#"\\\\Users\\\\[^/]+\\\\"#).unwrap(), "\\\\Users\\\\*****\\\\"),
    ]
});

pub fn replace(string: &str) -> Cow<'_, str> {
    let mut replaced = Cow::Borrowed(string);
    for (regex, replacement) in &*REPLACEMENTS {
        if let Cow::Owned(new) = regex.replace_all(&replaced, *replacement) {
            replaced = Cow::Owned(new);
        }
    }
    replaced
}

pub fn start_game_output(stdout: PipeReader, stderr: Option<PipeReader>, frontend: FrontendHandle) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    frontend.send(MessageToFrontend::CreateGameOutputWindow { receiver });

    if let Some(stderr) = stderr {
        let sender = sender.clone();
        std::thread::spawn(move || {
            let mut raw_text = String::new();
            let mut reader = BufReader::new(stderr);

            loop {
                match reader.read_line(&mut raw_text) {
                    Err(e) => panic!("Error while reading stderr: {:?}", e),
                    Ok(0) => {
                        return; // EOF
                    },
                    Ok(_) => {
                        let replaced = replace(&*raw_text);
                        let replaced = replaced.trim_end();

                        #[cfg(debug_assertions)]
                        if replaced.contains('\n') {
                            panic!("Line contains newline: {replaced:?}")
                        }

                        let res = sender.send(GameOutputMsg {
                            time: Utc::now().timestamp_millis(),
                            level: GameOutputLogLevel::Error,
                            text: Arc::new([replaced.into()]),
                        });
                        if res.is_err() {
                            return; // Window closed
                        }
                        raw_text.clear();
                    },
                }
            }
        });
    }

    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut log_reader = LogReader {
            stack: Vec::new(),
            sender: sender.clone(),
            empty_message: "<empty>".into()
        };
        let mut log_input = LogInput {
            buffer: Vec::new(),
            reader,
        };

        #[cfg(debug_assertions)]
        let result = {
            let panic_result = std::panic::catch_unwind(move || {
                log_reader.handle_output(&mut log_input)
            });
            match panic_result {
                Ok(result) => result,
                Err(panic_error) => {
                    let panic_error_str = match panic_error.downcast::<&str>() {
                        Ok(str) => String::from(*str),
                        Err(panic_error) => match panic_error.downcast::<String>() {
                            Ok(string) => *string,
                            Err(_) => "unable to convert panic message to &str".to_string(),
                        },
                    };

                    let panic_message = format!("(Pandora) There was an error while reading the log: {panic_error_str}");

                    _ = sender.send(GameOutputMsg {
                        time: Utc::now().timestamp_millis(),
                        level: GameOutputLogLevel::Fatal,
                        text: panic_message.lines().map(Arc::from).collect::<Arc<[_]>>(),
                    });
                    return;
                },
            }
        };
        #[cfg(not(debug_assertions))]
        let result = log_reader.handle_output(&mut log_input);

        if let Err(HandleOutputError::ReceiverClosed) = result {
            return;
        }

        if let Err(error) = result {
            let error_message = format!("(Pandora) There was an error while reading the log: {error}");

            _ = sender.send(GameOutputMsg {
                time: Utc::now().timestamp_millis(),
                level: GameOutputLogLevel::Fatal,
                text: error_message.lines().map(Arc::from).collect::<Arc<[_]>>(),
            });
        }
    });
}

#[derive(Error, Debug)]
enum HandleOutputError {
    #[error("An I/O error occurred:\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("Unable to convert text to UTF-8:\n{0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("Unexpected Eof")]
    UnexpectedEof,
    #[error("Invalid CDATA")]
    InvalidCdata,
    #[error("Invalid Comment")]
    InvalidComment,
    #[error("Unmatched element")]
    UnmatchedElement(String),
    #[error("Receiver closed")]
    ReceiverClosed,
}

struct LogReader {
    stack: Vec<LogOutputState>,
    sender: tokio::sync::mpsc::UnboundedSender<GameOutputMsg>,
    empty_message: Arc<str>,
}

struct LogInput {
    buffer: Vec<u8>,
    reader: BufReader<PipeReader>
}

#[derive(Debug)]
enum LogOutputState {
    Event {
        timestamp: Option<i64>,
        level: Option<GameOutputLogLevel>,
        text: Option<Arc<str>>,
        throwable: Option<Arc<str>>,
    },
    Message {
        content: Option<Arc<str>>,
    },
    Throwable {
        content: Option<Arc<str>>,
    },
    Unknown,
}

#[derive(PartialEq, Eq)]
enum ReadAttributesForElement {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedAttributeKey {
    Logger,
    Timestamp,
    Level,
    Thread,
    Unknown,
}

impl LogReader {
    pub fn handle_output(&mut self, input: &mut LogInput) -> Result<(), HandleOutputError> {
        loop {
            let available = input.reader.fill_buf()?;
            if available.is_empty() {
                return Ok(());
            }

            // If we are inside XML, only try to read XML
            if !self.stack.is_empty() {
                let Some(index) = memchr::memchr(b'<', available) else {
                    let read = available.len();
                    input.reader.consume(read);
                    continue;
                };

                input.reader.consume(index+1);
                self.read_markup(input)?;

                continue;
            }

            // Try to read either XML or a raw line
            let Some(index) = memchr::memchr2(b'\n', b'<', available) else {
                let buffer_contains_non_whitespace = !available.trim_ascii().is_empty();

                input.buffer.extend_from_slice(available);
                let read = available.len();
                input.reader.consume(read);

                if buffer_contains_non_whitespace {
                    self.read_rest_of_line(input)?;
                }

                continue;
            };

            if available[index] == b'\n' {
                self.finish_text(&available[..index], &mut input.buffer)?;
                input.reader.consume(index+1);
            } else if !available[..index].trim_ascii().is_empty() {
                // Line contains non-whitespace before <, treat as a literal line instead of markup
                if let Some(new_index) = memchr::memchr(b'\n', &available[index..]) {
                    self.finish_text(&available[..index+new_index], &mut input.buffer)?;
                    input.reader.consume(index+new_index+1);
                    continue;
                }

                input.buffer.extend_from_slice(available);
                let read = available.len();
                input.reader.consume(read);

                self.read_rest_of_line(input)?;
            } else {
                input.buffer.clear();
                input.reader.consume(index+1);
                self.read_markup(input)?;
            }
        }
    }

    fn read_markup(&mut self, input: &mut LogInput) -> Result<(), HandleOutputError> {
        let available = input.reader.fill_buf()?;
        if available.is_empty() {
            return Err(HandleOutputError::UnexpectedEof);
        }

        let peeked = available[0];
        if peeked == b'!' {
            input.reader.consume(1);
            self.read_bang(input)?;
        } else if peeked == b'/' {
            input.reader.consume(1);
            self.read_end_element(input)?;
        } else if peeked == b'?' {
            input.reader.consume(1);
            self.read_processing_instruction(input)?;
        } else {
            self.read_element(input)?;
        }

        debug_assert!(input.buffer.is_empty());
        Ok(())
    }

    fn read_bang(&mut self, input: &mut LogInput) -> Result<(), HandleOutputError> {
        debug_assert!(input.buffer.is_empty());

        let available = input.reader.fill_buf()?;
        if available.is_empty() {
            return Err(HandleOutputError::UnexpectedEof);
        }

        match available[0] {
            b'[' => {
                // <![CDATA[..]]>

                loop {
                    let available = input.reader.fill_buf()?;
                    if available.is_empty() {
                        return Err(HandleOutputError::UnexpectedEof);
                    }

                    let Some(index) = memchr::memchr(b'>', available) else {
                        input.buffer.extend_from_slice(available);
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    if available.len() >= 3 && available[..index+1].ends_with(b"]]>") {
                        let remaining_text = &available[..index-2];
                        if input.buffer.is_empty() {
                            self.apply_cdata(remaining_text)?;
                        } else {
                            input.buffer.extend_from_slice(remaining_text);
                            self.apply_cdata(&input.buffer)?;
                            input.buffer.clear();
                        }
                        input.reader.consume(index+1);
                        return Ok(());
                    }

                    input.buffer.extend_from_slice(&available[..index+1]);
                    input.reader.consume(index+1);

                    if input.buffer.len() >= 3 && input.buffer.ends_with(b"]]>") {
                        self.apply_cdata(&input.buffer[..input.buffer.len()-3])?;
                        input.buffer.clear();
                        return Ok(());
                    }
                }
            },
            b'-' => {
                // <!-- --> (Comment)

                // Check for start sequence
                if available.len() >= 2 {
                    if available[1] != b'-' {
                        return Err(HandleOutputError::InvalidComment);
                    }
                    input.reader.consume(2);
                } else {
                    input.reader.consume(1);

                    let available = input.reader.fill_buf()?;
                    if available.is_empty() {
                        return Err(HandleOutputError::UnexpectedEof);
                    }

                    if available[0] != b'-' {
                        return Err(HandleOutputError::InvalidComment);
                    }

                    input.reader.consume(1);
                }

                let mut partial_end_sequence = 0; // 1 = "-", 2 = "--"

                loop {
                    let available = input.reader.fill_buf()?;
                    if available.is_empty() {
                        return Err(HandleOutputError::UnexpectedEof);
                    }

                    let Some(index) = memchr::memchr(b'>', available) else {
                        if available.len() == 1 && available[0] == b'-' && partial_end_sequence == 1 {
                            partial_end_sequence = 2; // Case when the buffer size is exactly 1 (we need 3 reads)
                        } else if available.len() >= 2 && &available[available.len()-2..] == b"--" {
                            partial_end_sequence = 2;
                        } else if available[available.len()-1] == b'-' {
                            partial_end_sequence = 1;
                        } else {
                            partial_end_sequence = 0;
                        }
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    let success = if index == 0 && partial_end_sequence == 2 {
                        true
                    } else if index == 1 && partial_end_sequence == 1 && available[0] == b'-' {
                        true
                    } else if index >= 2 && &available[index-2..index] == b"--" {
                        true
                    } else {
                        false
                    };

                    partial_end_sequence = 0;
                    input.reader.consume(index+1);

                    if success {
                        return Ok(());
                    }
                }
            },
            b'D' | b'd' => {
                // DOCTYPE
                Self::skip_balanced_angle_brackets(1, input)?;
            },
            _ => {
                if cfg!(debug_assertions) {
                    panic!("Unknown bang type for character: {}", available[0])
                } else {
                    Self::skip_balanced_angle_brackets(1, input)?;
                }
            }
        }

        Ok(())
    }

    fn read_processing_instruction(&mut self, input: &mut LogInput) -> Result<(), HandleOutputError> {
        let mut ended_with_question_mark = false;

        loop {
            let available = input.reader.fill_buf()?;
            if available.is_empty() {
                return Err(HandleOutputError::UnexpectedEof);
            }

            let Some(index) = memchr::memchr(b'>', available) else {
                ended_with_question_mark = available[available.len()-1] == b'?';
                let read = available.len();
                input.reader.consume(read);
                continue;
            };


            let success = if index == 0 && ended_with_question_mark {
                true
            } else if index >= 1 && available[index-1] == b'?' {
                true
            } else {
                false
            };

            ended_with_question_mark = false;
            input.reader.consume(index+1);

            if success {
                return Ok(());
            }
        }
    }

    fn skip_balanced_angle_brackets(mut depth: usize, input: &mut LogInput) -> Result<(), HandleOutputError> {
        loop {
            let available = input.reader.fill_buf()?;
            if available.is_empty() {
                return Err(HandleOutputError::UnexpectedEof);
            }

            let Some(index) = memchr::memchr2(b'<', b'>', available) else {
                let read = available.len();
                input.reader.consume(read);
                continue;
            };

            let last = available[index];
            input.reader.consume(index+1);

            if last == b'<' {
                depth += 1;
            } else {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
        }
    }

    fn read_element(&mut self, input: &mut LogInput) -> Result<(), HandleOutputError> {
        debug_assert!(input.buffer.is_empty());

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum ElementParseState {
            ReadingName,
            ReadingKey,
            ReadingValue(NamedAttributeKey),
            ReadingValueSingleQuoted(NamedAttributeKey),
            ReadingValueDoubleQuoted(NamedAttributeKey),
            Skip,
            SkipSingleQuotes,
            SkipDoubleQuotes,
        }
        let mut state = ElementParseState::ReadingName;

        let mut skip_had_slash_last = false;

        loop {
            let available = input.reader.fill_buf()?;
            if available.is_empty() {
                return Err(HandleOutputError::UnexpectedEof);
            }

            match state {
                ElementParseState::ReadingName => {
                    let end = available.iter().position(|b| is_xml_whitespace(*b) || *b == b'>');
                    let Some(end) = end else {
                        input.buffer.extend_from_slice(available);
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    let terminator = available[end];

                    let name = if input.buffer.is_empty() {
                        &available[..end]
                    } else {
                        input.buffer.extend_from_slice(&available[..end]);
                        &input.buffer
                    };

                    if name.is_empty() {
                        input.buffer.clear();
                        input.reader.consume(end);
                        state = ElementParseState::Skip;
                        continue;
                    }

                    if terminator == b'>' && name[name.len()-1] == b'/' {
                        // Skip auto-closing tags
                        input.buffer.clear();
                        input.reader.consume(end+1);
                        return Ok(());
                    }

                    let read_attributes = self.apply_new_element(name);

                    if terminator == b'>' {
                        input.buffer.clear();
                        input.reader.consume(end+1);
                        return Ok(());
                    }

                    input.buffer.clear();
                    input.reader.consume(end);

                    if read_attributes == ReadAttributesForElement::Yes {
                        self.skip_whitespace(input)?;
                        state = ElementParseState::ReadingKey;
                    } else {
                        state = ElementParseState::Skip;
                    }
                },
                ElementParseState::ReadingKey => {
                    let end = available.iter().position(|b| is_xml_whitespace(*b) || *b == b'>' ||
                        *b == b'\'' || *b == b'"' || *b == b'=');
                    let Some(end) = end else {
                        input.buffer.extend_from_slice(available);
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    let terminator = available[end];

                    if terminator == b'>' && end == 0 {
                        if !input.buffer.is_empty() && input.buffer.ends_with(b"/") {
                            self.stack.pop();
                        }
                        input.buffer.clear();
                        input.reader.consume(end+1);
                        return Ok(());
                    } else if terminator == b'>' && available[end-1] == b'/' {
                        self.stack.pop();
                        input.buffer.clear();
                        input.reader.consume(end+1);
                        return Ok(());
                    } else if terminator != b'=' {
                        if cfg!(debug_assertions) {
                            panic!("Expected eq after element key");
                        } else {
                            state = ElementParseState::Skip;
                            input.buffer.clear();
                            input.reader.consume(end);
                            continue;
                        }
                    }

                    let name = if input.buffer.is_empty() {
                        &available[..end]
                    } else {
                        input.buffer.extend_from_slice(&available[..end]);
                        &input.buffer
                    };

                    let key = match name {
                        b"logger" => NamedAttributeKey::Logger,
                        b"timestamp" => NamedAttributeKey::Timestamp,
                        b"level" => NamedAttributeKey::Level,
                        b"thread" => NamedAttributeKey::Thread,
                        _ => {
                            if cfg!(debug_assertions) {
                                panic!("Unknown element attribute key {:?}", str::from_utf8(name));
                            } else {
                                NamedAttributeKey::Unknown
                            }
                        }
                    };

                    input.buffer.clear();
                    input.reader.consume(end+1); // +1 to skip '=' as well

                    state = ElementParseState::ReadingValue(key);
                },
                ElementParseState::ReadingValue(key) => {
                    if available[0] == b'\'' {
                        input.reader.consume(1);
                        state = ElementParseState::ReadingValueSingleQuoted(key);
                    } else if available[0] == b'"' {
                        input.reader.consume(1);
                        state = ElementParseState::ReadingValueDoubleQuoted(key);
                    } else if cfg!(debug_assertions) {
                        panic!("Expected single or double quote after eq");
                    } else {
                        state = ElementParseState::Skip;
                    }
                },
                ElementParseState::ReadingValueDoubleQuoted(key) | ElementParseState::ReadingValueSingleQuoted(key) => {
                    let needle = match state {
                        ElementParseState::ReadingValueDoubleQuoted(_) => b'"',
                        ElementParseState::ReadingValueSingleQuoted(_) => b'\'',
                        _ => unreachable!()
                    };

                    let end = memchr(needle, available);
                    let Some(end) = end else {
                        input.buffer.extend_from_slice(available);
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    let value = if input.buffer.is_empty() {
                        &available[..end]
                    } else {
                        input.buffer.extend_from_slice(&available[..end]);
                        &input.buffer
                    };

                    self.apply_attribute_key_value(key, value);

                    input.buffer.clear();
                    input.reader.consume(end+1); // +1 to skip '=' as well

                    self.skip_whitespace(input)?;
                    state = ElementParseState::ReadingKey;
                },
                ElementParseState::Skip => {
                    let Some(end) = memchr::memchr3(b'>', b'\'', b'"', available) else {
                        skip_had_slash_last = available.ends_with(b"/");
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    let terminator = available[end];

                    if terminator == b'\'' {
                        skip_had_slash_last = false;
                        state = ElementParseState::SkipSingleQuotes;
                    } else if terminator == b'"' {
                        skip_had_slash_last = false;
                        state = ElementParseState::SkipDoubleQuotes;
                    } else {
                        if end == 0 && skip_had_slash_last {
                            self.stack.pop();
                        } else if end >= 1 && available[end-1] == b'/' {
                            self.stack.pop();
                        }
                        input.reader.consume(end+1);
                        return Ok(());
                    }

                    input.reader.consume(end+1);
                },
                ElementParseState::SkipSingleQuotes => {
                    let Some(end) = memchr::memchr(b'\'', available) else {
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    input.reader.consume(end+1);
                    state = ElementParseState::Skip;
                },
                ElementParseState::SkipDoubleQuotes => {
                    let Some(end) = memchr::memchr(b'"', available) else {
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    input.reader.consume(end+1);
                    state = ElementParseState::Skip;
                },
            }
        }
    }

    fn read_end_element(&mut self, input: &mut LogInput) -> Result<(), HandleOutputError> {
        debug_assert!(input.buffer.is_empty());

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum ElementParseState {
            ReadingName,
            Skip,
            SkipSingleQuotes,
            SkipDoubleQuotes,
        }
        let mut state = ElementParseState::ReadingName;

        loop {
            let available = input.reader.fill_buf()?;
            if available.is_empty() {
                return Err(HandleOutputError::UnexpectedEof);
            }

            match state {
                ElementParseState::ReadingName => {
                    let end = available.iter().position(|b| is_xml_whitespace(*b) || *b == b'>');
                    let Some(end) = end else {
                        input.buffer.extend_from_slice(available);
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    let name = if input.buffer.is_empty() {
                        &available[..end]
                    } else {
                        input.buffer.extend_from_slice(&available[..end]);
                        &input.buffer
                    };

                    self.apply_end_element(name)?;

                    input.buffer.clear();
                    input.reader.consume(end);
                    state = ElementParseState::Skip;
                },
                ElementParseState::Skip => {
                    let Some(end) = memchr::memchr3(b'>', b'\'', b'"', available) else {
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    let terminator = available[end];
                    input.reader.consume(end+1);

                    if terminator == b'\'' {
                        state = ElementParseState::SkipSingleQuotes;
                    } else if terminator == b'"' {
                        state = ElementParseState::SkipDoubleQuotes;
                    } else {
                        return Ok(());
                    }
                },
                ElementParseState::SkipSingleQuotes => {
                    let Some(end) = memchr::memchr(b'\'', available) else {
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    input.reader.consume(end+1);
                    state = ElementParseState::Skip;
                },
                ElementParseState::SkipDoubleQuotes => {
                    let Some(end) = memchr::memchr(b'"', available) else {
                        let read = available.len();
                        input.reader.consume(read);
                        continue;
                    };

                    input.reader.consume(end+1);
                    state = ElementParseState::Skip;
                },
            }
        }
    }

    fn apply_cdata(&mut self, cdata: &[u8]) -> Result<(), HandleOutputError> {
        let Some(cdata) = cdata.strip_prefix(b"[CDATA[") else {
            return Err(HandleOutputError::InvalidCdata);
        };

        let str = match str::from_utf8(cdata) {
            Ok(str) => Cow::Borrowed(str),
            Err(err) => Cow::Owned(format!("{}", HandleOutputError::Utf8Error(err))),
        };

        match self.stack.last_mut() {
            None => {
                self.send_raw_text(&str)?;
            },
            Some(LogOutputState::Message { content }) => {
                *content = Some(str.into());
            },
            Some(LogOutputState::Throwable { content }) => {
                *content = Some(str.into());
            },
            last => {
                if cfg!(debug_assertions) {
                    panic!("Unexpected cdata on {:?}", last);
                }
            }
        }
        Ok(())
    }

    fn apply_new_element(&mut self, name: &[u8]) -> ReadAttributesForElement {
        match self.stack.last_mut() {
            None => {
                if name == b"log4j:Event" {
                    self.stack.push(LogOutputState::Event {
                        timestamp: None,
                        level: None,
                        text: None,
                        throwable: None
                    });
                    return ReadAttributesForElement::Yes;
                } else if cfg!(debug_assertions) {
                    panic!("Unexpected element {:?} on {:?}", str::from_utf8(name), self.stack.last_mut());
                } else {
                    self.stack.push(LogOutputState::Unknown);
                }
            },
            Some(LogOutputState::Event { .. }) => {
                if name == b"log4j:Message" {
                    self.stack.push(LogOutputState::Message { content: None });
                } else if name == b"log4j:Throwable" {
                    self.stack.push(LogOutputState::Throwable { content: None });
                } else if cfg!(debug_assertions) {
                    panic!("Unexpected element {:?} on {:?}", str::from_utf8(name), self.stack.last_mut());
                } else {
                    self.stack.push(LogOutputState::Unknown);
                }
            },
            _ => {
                if cfg!(debug_assertions) {
                    panic!("Unexpected element {:?} on {:?}", str::from_utf8(name), self.stack.last_mut());
                } else {
                    self.stack.push(LogOutputState::Unknown);
                }
            }
        }
        ReadAttributesForElement::No
    }

    fn apply_end_element(&mut self, name: &[u8]) -> Result<(), HandleOutputError> {
        match self.stack.last_mut() {
            Some(LogOutputState::Event { .. }) => {
                if name != b"log4j:Event" {
                    return Err(HandleOutputError::UnmatchedElement(str::from_utf8(name)?.into()));
                }

                let Some(LogOutputState::Event { timestamp, level, mut text, mut throwable }) = self.stack.pop() else {
                    unreachable!()
                };
                let mut lines = Vec::new();

                if let Some(text) = text.as_mut() {
                    let replaced = replace(&**text);
                    if let Cow::Owned(replaced) = replaced {
                        *text = replaced.into();
                    }
                }
                if let Some(throwable) = throwable.as_mut() {
                    let replaced = replace(&**throwable);
                    if let Cow::Owned(replaced) = replaced {
                        *throwable = replaced.into();
                    }
                }

                if let Some(text) = &text {
                    let mut split = text.split('\n');
                    if let Some(first) = split.next() && let Some(second) = split.next() {
                        lines.push(Arc::from(first.trim_end()));
                        lines.push(Arc::from(second.trim_end()));
                        for next in split {
                            lines.push(Arc::from(next.trim_end()));
                        }
                    }
                }
                if let Some(throwable) = &throwable {
                    let mut split = throwable.split('\n');
                    if let Some(first) = split.next() && let Some(second) = split.next() {
                        if let Some(text) = text.take() && lines.is_empty() {
                            lines.push(text);
                        }

                        lines.push(Arc::from(first.trim_end()));
                        lines.push(Arc::from(second.trim_end()));
                        for next in split {
                            lines.push(Arc::from(next.trim_end()));
                        }
                    }
                }

                let final_lines: Arc<[Arc<str>]> = if !lines.is_empty() {
                    lines.into()
                } else if let Some(text) = text.take() {
                    if let Some(throwable) = throwable.take() {
                        Arc::new([text, throwable])
                    } else {
                        Arc::new([text])
                    }
                } else if let Some(throwable) = throwable {
                    Arc::new([throwable])
                } else {
                    Arc::new([self.empty_message.clone()])
                };
                let res = self.sender.send(GameOutputMsg {
                    time: timestamp.unwrap_or(Utc::now().timestamp_millis()),
                    level: level.unwrap_or(GameOutputLogLevel::Other),
                    text: final_lines,
                });
                if res.is_err() {
                    return Err(HandleOutputError::ReceiverClosed);
                }
            },
            Some(LogOutputState::Message { .. }) => {
                if name != b"log4j:Message" {
                    return Err(HandleOutputError::UnmatchedElement(str::from_utf8(name)?.into()));
                }

                let Some(LogOutputState::Message { content }) = self.stack.pop() else {
                    unreachable!()
                };

                if let Some(LogOutputState::Event { text, .. }) = self.stack.last_mut() {
                    *text = content;
                } else {
                    panic!("log4j:Message should only be inside log4j:Event");
                }
            },
            Some(LogOutputState::Throwable { .. }) => {
                if name != b"log4j:Throwable" {
                    return Err(HandleOutputError::UnmatchedElement(str::from_utf8(name)?.into()));
                }

                let Some(LogOutputState::Throwable { content }) = self.stack.pop() else {
                    unreachable!()
                };

                if let Some(LogOutputState::Event { throwable, .. }) = self.stack.last_mut() {
                    *throwable = content;
                } else {
                    panic!("log4j:Throwable should only be inside log4j:Event");
                }
            },
            Some(LogOutputState::Unknown) => {
                _ = self.stack.pop();
            }
            None => {
                return Err(HandleOutputError::UnmatchedElement(str::from_utf8(name)?.into()));
            },
        }
        Ok(())
    }

    fn apply_attribute_key_value(&mut self, key: NamedAttributeKey, value: &[u8]) {
        match self.stack.last_mut() {
            Some(LogOutputState::Event { timestamp, level, .. }) => {
                match key {
                    NamedAttributeKey::Logger => {
                        // Ignore
                    },
                    NamedAttributeKey::Timestamp => {
                        let Ok(value) = str::from_utf8(&value) else {
                            return;
                        };
                        if let Ok(parsed) = value.parse() {
                            *timestamp = Some(parsed);
                        }
                    },
                    NamedAttributeKey::Level => {
                        *level = Some(match value {
                            b"FATAL" => GameOutputLogLevel::Fatal,
                            b"ERROR" => GameOutputLogLevel::Error,
                            b"WARN" => GameOutputLogLevel::Warn,
                            b"INFO" => GameOutputLogLevel::Info,
                            b"DEBUG" => GameOutputLogLevel::Debug,
                            b"TRACE" => GameOutputLogLevel::Trace,
                            _ => GameOutputLogLevel::Other,
                        });
                    },
                    NamedAttributeKey::Thread => {
                        // Ignore
                    }
                    _ => {
                        if cfg!(debug_assertions) {
                            panic!("Unexpected attribute {:?} on {:?}", key, self.stack.last_mut());
                        }
                    },
                }
            },
            _ => {
                if cfg!(debug_assertions) {
                    panic!("Unexpected attribute {:?} on {:?}", key, self.stack.last_mut());
                }
            }
        }
    }

    fn skip_whitespace(&mut self, input: &mut LogInput) -> Result<(), HandleOutputError> {
        loop {
            let available = input.reader.fill_buf()?;
            if available.is_empty() {
                return Ok(());
            }

            let end = available.iter().position(|b| !is_xml_whitespace(*b));
            if let Some(end) = end {
                input.reader.consume(end);
                return Ok(());
            } else {
                let read = available.len();
                input.reader.consume(read);
            }
        }
    }

    fn read_rest_of_line(&mut self, input: &mut LogInput) -> Result<(), HandleOutputError> {
        loop {
            let available = input.reader.fill_buf()?;

            if available.is_empty() {
                self.finish_text(b"", &mut input.buffer)?;
                return Ok(());
            }

            if let Some(index) = memchr::memchr(b'\n', available) {
                self.finish_text(&available[..index], &mut input.buffer)?;
                return Ok(());
            } else {
                input.buffer.extend_from_slice(available);
                let read = available.len();
                input.reader.consume(read);
            }
        }
    }

    fn finish_text(&mut self, remaining: &[u8], buffer: &mut Vec<u8>) -> Result<(), HandleOutputError> {
        let line = if buffer.is_empty() {
            str::from_utf8(remaining)
        } else {
            buffer.extend_from_slice(remaining);
            str::from_utf8(&buffer)
        };

        let result = match line {
            Ok(str) => self.send_raw_text(&str),
            Err(err) => self.send_raw_text(&format!("{}", HandleOutputError::Utf8Error(err))),
        };

        buffer.clear();

        result
    }

    fn send_raw_text(&mut self, text: &str) -> Result<(), HandleOutputError> {
        if text.trim_ascii().is_empty() {
            return Ok(());
        }

        let res = self.sender.send(GameOutputMsg {
            time: Utc::now().timestamp_millis(),
            level: GameOutputLogLevel::Info,
            text: text.lines().map(Arc::from).collect::<Arc<[_]>>(),
        });
        if res.is_err() {
            return Err(HandleOutputError::ReceiverClosed);
        }

        Ok(())
    }
}

fn is_xml_whitespace(byte: u8) -> bool {
    matches!(byte, b'\r' | b'\n' | b'\t' | b' ')
}
