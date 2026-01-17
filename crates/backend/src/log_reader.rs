use std::{
    borrow::Cow,
    io::{BufRead, BufReader},
    process::{ChildStderr, ChildStdout},
    sync::{atomic::AtomicUsize, Arc},
};

use bridge::{
    game_output::GameOutputLogLevel, handle::FrontendHandle, keep_alive::KeepAlive, message::MessageToFrontend,
};
use chrono::Utc;
use memchr::memchr;
use once_cell::sync::Lazy;
use regex::Regex;
use thiserror::Error;

static GAME_OUTPUT_ID: AtomicUsize = AtomicUsize::new(0);
static REPLACEMENTS: Lazy<[(Regex, &'static str); 7]> = Lazy::new(|| {
    [
        (regex::Regex::new(r#"SignedJWT: [^\s]+"#).unwrap(), "SignedJWT: *****"),
        (regex::Regex::new(r#"Session ID is [^\s)]+"#).unwrap(), "Session ID is *****"),
        (regex::Regex::new(r#"--accessToken, [^\s,]+"#).unwrap(), "--accessToken, *****"),
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

pub fn start_game_output(
    stdout: ChildStdout,
    stderr: Option<ChildStderr>,
    sender: FrontendHandle,
    buffer: Arc<std::sync::Mutex<Vec<(i64, GameOutputLogLevel, Arc<[Arc<str>]>)>>>,
) -> (usize) {
    let id = GAME_OUTPUT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let keepalive = KeepAlive::new();
    let keepalive_handle = keepalive.create_handle();

    sender.send(MessageToFrontend::CreateGameOutputWindow { id, keep_alive: keepalive });

    if let Some(stderr) = stderr {
        let sender = sender.clone();
        let keepalive_handle = keepalive_handle.clone();
        let buffer = buffer.clone();

        std::thread::spawn(move || {
            let mut rawtext = String::new();
            let mut reader = BufReader::new(stderr);

            while keepalive_handle.is_alive() {
                match reader.read_line(&mut rawtext) {
                    Err(e) => panic!("Error while reading stderr: {e:?}"),
                    Ok(0) => break,
                    Ok(_) => {
                        let replaced = replace(&rawtext);
                        let time = Utc::now().timestamp_millis();
                        let level = GameOutputLogLevel::Error;
                        let text = Arc::new([replaced.trim_end().into()]);

                        buffer.lock().unwrap_or_else(|e| e.into_inner()).push((time, level, text.clone()));

                        
                        sender.send(MessageToFrontend::AddGameOutput { id, time, level, text });

                        rawtext.clear();
                    }
                }
            }
        });
    }

    let buffer_clone = buffer.clone();

    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut log_reader = LogReader {
            stack: Vec::new(),
            id,
            sender: sender.clone(),
            empty_message: "empty".into(),
            buffer: buffer_clone,
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
                            Err(_) => "unable to convert panic message to str".to_string(),
                        },
                    };
                    let time = Utc::now().timestamp_millis();
                    let level = GameOutputLogLevel::Fatal;
                    let text = Arc::new([format!("[Pandora] There was an error while reading the log: {panic_error_str}").into()]);
                    sender.send(MessageToFrontend::AddGameOutput { id, time, level, text });
                    return;
                }
            }
        };

        #[cfg(not(debug_assertions))]
        let result = log_reader.handle_output(&mut log_input);

        if let Err(error) = result {
            let time = Utc::now().timestamp_millis();
            let level = GameOutputLogLevel::Fatal;
            let text = Arc::new([format!("[Pandora] There was an error while reading the log: {error}").into()]);
            sender.send(MessageToFrontend::AddGameOutput { id, time, level, text });
        }
    });

    id
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
}

struct LogReader {
    stack: Vec<LogOutputState>,
    id: usize,
    sender: FrontendHandle,
    empty_message: Arc<str>,
    buffer: Arc<std::sync::Mutex<Vec<(i64, GameOutputLogLevel, Arc<[Arc<str>]>)>>>,
}

struct LogInput {
    buffer: Vec<u8>,
    reader: BufReader<ChildStdout>
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

                let mut partial_end_sequence = 0;

                loop {
                    let available = input.reader.fill_buf()?;
                    if available.is_empty() {
                        return Err(HandleOutputError::UnexpectedEof);
                    }

                    let Some(index) = memchr::memchr(b'>', available) else {
                        if available.len() == 1 && available[0] == b'-' && partial_end_sequence == 1 {
                            partial_end_sequence = 2;
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
                    input.reader.consume(end+1);

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
                    input.reader.consume(end+1);

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
        let str = str::from_utf8(cdata)?;

        match self.stack.last_mut() {
            None => self.send_raw_text(str)?,
            Some(LogOutputState::Message { content }) => {
                *content = Some(str.into());
            }
            Some(LogOutputState::Throwable { content }) => {
                *content = Some(str.into());
            }
            last => {
                #[cfg(debug_assertions)]
                panic!("Unexpected cdata on {last:?}");
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
            Some(LogOutputState::Event { .. }) if name == b"log4j:Event" => {
                let Some(LogOutputState::Event { timestamp, level, mut text, mut throwable }) = self.stack.pop() else {
                    unreachable!();
                };

                let mut lines = Vec::new();

                if let Some(text) = text.as_mut() {
                    let replaced = replace(text);
                    if let Cow::Owned(replaced) = replaced {
                        *text = replaced.into();
                    }
                }

                if let Some(throwable) = throwable.as_mut() {
                    let replaced = replace(throwable);
                    if let Cow::Owned(replaced) = replaced {
                        *throwable = replaced.into();
                    }
                }

                if let Some(text) = &text {
                    let mut split = text.split('\n');
                    if let Some(first) = split.next() {
                        if let Some(second) = split.next() {
                            lines.push(Arc::from(first.trim_end()));
                            lines.push(Arc::from(second.trim_end()));
                            for next in split {
                                lines.push(Arc::from(next.trim_end()));
                            }
                        }
                    }
                }

                if let Some(throwable) = &throwable {
                    let mut split = throwable.split('\n');
                    if let Some(first) = split.next() {
                        if let Some(second) = split.next() {
                            if let Some(text) = text.take() {
                                if lines.is_empty() {
                                    lines.push(text);
                                }
                            }
                            lines.push(Arc::from(first.trim_end()));
                            lines.push(Arc::from(second.trim_end()));
                            for next in split {
                                lines.push(Arc::from(next.trim_end()));
                            }
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

                let time = timestamp.unwrap_or(Utc::now().timestamp_millis());
                let level = level.unwrap_or(GameOutputLogLevel::Other);

                self.buffer.lock().unwrap_or_else(|e| e.into_inner()).push((time, level, final_lines.clone()));
                
                self.sender.send(MessageToFrontend::AddGameOutput {
                        id: self.id,
                        time,
                        level,
                        text: final_lines,
                    });
                }
            Some(LogOutputState::Message { .. }) if name == b"log4j:Message" => {
                let Some(LogOutputState::Message { content }) = self.stack.pop() else {
                    unreachable!();
                };
                if let Some(LogOutputState::Event { text, .. }) = self.stack.last_mut() {
                    *text = content;
                } else {
                    panic!("log4j:Message should only be inside log4j:Event");
                }
            }
            Some(LogOutputState::Throwable { .. }) if name == b"log4j:Throwable" => {
                let Some(LogOutputState::Throwable { content }) = self.stack.pop() else {
                    unreachable!();
                };
                if let Some(LogOutputState::Event { throwable, .. }) = self.stack.last_mut() {
                    *throwable = content;
                } else {
                    panic!("log4j:Throwable should only be inside log4j:Event");
                }
            }
            Some(LogOutputState::Unknown) => {
                self.stack.pop();
            }
            None => {
                return Err(HandleOutputError::UnmatchedElement(str::from_utf8(name)?.into()));
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_attribute_key_value(&mut self, key: NamedAttributeKey, value: &[u8]) {
        match self.stack.last_mut() {
            Some(LogOutputState::Event { timestamp, level, .. }) => {
                match key {
                    NamedAttributeKey::Logger => {
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
            str::from_utf8(remaining)?
        } else {
            buffer.extend_from_slice(remaining);
            str::from_utf8(&buffer)?
        };

        let result = self.send_raw_text(line);

        buffer.clear();

        result
    }

    fn send_raw_text(&mut self, line: &str) -> Result<(), HandleOutputError> {
        if line.trim_ascii().is_empty() {
            return Ok(());
        }

        let time = Utc::now().timestamp_millis();
        let level = GameOutputLogLevel::Info;
        let text = Arc::new([line.into()]);
        
        self.sender.send(MessageToFrontend::AddGameOutput {
                id: self.id,
                time,
                level,
                text,
            });
        

        Ok(())
    }

}

fn is_xml_whitespace(byte: u8) -> bool {
    matches!(byte, b'\r' | b'\n' | b'\t' | b' ')
}
