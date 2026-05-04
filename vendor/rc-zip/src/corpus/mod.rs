#![allow(missing_docs)]

//! A corpus of zip files for testing.

use std::{fs::File, path::PathBuf};

use chrono::{DateTime, FixedOffset, TimeZone, Timelike, Utc};
use temp_dir::TempDir;
use tracing::span;

use crate::{
    encoding::Encoding,
    error::Error,
    parse::{Archive, Entry, EntryKind},
};

pub struct Case {
    pub name: &'static str,
    pub expected_encoding: Option<Encoding>,
    pub comment: Option<&'static str>,
    pub files: Files,
    pub error: Option<Error>,
}

pub enum Files {
    ExhaustiveList(Vec<CaseFile>),
    NumFiles(usize),
}

impl Files {
    fn len(&self) -> usize {
        match self {
            Self::ExhaustiveList(list) => list.len(),
            Self::NumFiles(n) => *n,
        }
    }
}

impl Default for Case {
    fn default() -> Self {
        Self {
            name: "test.zip",
            expected_encoding: None,
            comment: None,
            files: Files::NumFiles(0),
            error: None,
        }
    }
}

/// This path may disappear on drop (if the zip is bz2-compressed), so be
/// careful
pub struct GuardedPath {
    pub path: PathBuf,
    _guard: Option<TempDir>,
}

impl Case {
    pub fn absolute_path(&self) -> GuardedPath {
        let path = zips_dir().join(self.name);
        if let Some(dec_name) = self.name.strip_suffix(".bz2") {
            let dir = TempDir::new().unwrap();
            let dec_path = dir.path().join(dec_name);
            std::io::copy(
                &mut File::open(&path).unwrap(),
                &mut bzip2::write::BzDecoder::new(File::create(&dec_path).unwrap()),
            )
            .unwrap();
            tracing::trace!("decompressed {} to {}", path.display(), dec_path.display());
            GuardedPath {
                path: dec_path,
                _guard: Some(dir),
            }
        } else {
            GuardedPath { path, _guard: None }
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        let gp = self.absolute_path();
        std::fs::read(gp.path).unwrap()
    }
}

pub struct CaseFile {
    pub name: &'static str,
    pub mode: Option<u32>,
    pub modified: Option<DateTime<Utc>>,
    pub content: FileContent,
}

pub enum FileContent {
    Unchecked,
    Bytes(Vec<u8>),
    File(&'static str),
}

impl Default for CaseFile {
    fn default() -> Self {
        Self {
            name: "default",
            mode: None,
            modified: None,
            content: FileContent::Unchecked,
        }
    }
}

pub fn zips_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("testdata")
}

fn time_zone(hours: i32) -> FixedOffset {
    FixedOffset::east_opt(hours * 3600).unwrap()
}

fn date(
    (year, month, day): (i32, u32, u32),
    (hour, min, sec): (u32, u32, u32),
    nsec: u32,
    offset: FixedOffset,
) -> Option<DateTime<Utc>> {
    Some(
        offset
            .with_ymd_and_hms(year, month, day, hour, min, sec)
            .single()?
            .with_nanosecond(nsec)?
            .into(),
    )
}

pub fn test_cases() -> Vec<Case> {
    vec![
        Case {
            name: "zip64.zip",
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "README",
                content: FileContent::Bytes(
                    "This small file is in ZIP64 format.\n".as_bytes().into(),
                ),
                modified: Some(date((2012, 8, 10), (14, 33, 32), 0, time_zone(0)).unwrap()),
                mode: Some(0o644),
            }]),
            ..Default::default()
        },
        Case {
            name: "test.zip",
            comment: Some("This is a zipfile comment."),
            expected_encoding: Some(Encoding::Utf8),
            files: Files::ExhaustiveList(vec![
                CaseFile {
                    name: "test.txt",
                    content: FileContent::Bytes("This is a test text file.\n".as_bytes().into()),
                    modified: Some(date((2010, 9, 5), (12, 12, 1), 0, time_zone(10)).unwrap()),
                    mode: Some(0o644),
                },
                CaseFile {
                    name: "gophercolor16x16.png",
                    content: FileContent::File("gophercolor16x16.png"),
                    modified: Some(date((2010, 9, 5), (15, 52, 58), 0, time_zone(10)).unwrap()),
                    mode: Some(0o644),
                },
            ]),
            ..Default::default()
        },
        Case {
            name: "cp-437.zip",
            expected_encoding: Some(Encoding::Cp437),
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "français",
                ..Default::default()
            }]),
            ..Default::default()
        },
        Case {
            name: "shift-jis.zip",
            expected_encoding: Some(Encoding::ShiftJis),
            files: Files::ExhaustiveList(vec![
                CaseFile {
                    name: "should-be-jis/",
                    ..Default::default()
                },
                CaseFile {
                    name: "should-be-jis/ot_運命のワルツﾈぞなぞ小さな楽しみ遊びま.longboi",
                    ..Default::default()
                },
            ]),
            ..Default::default()
        },
        Case {
            name: "utf8-winrar.zip",
            expected_encoding: Some(Encoding::Utf8),
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "世界",
                content: FileContent::Bytes(vec![]),
                modified: Some(date((2017, 11, 6), (21, 9, 27), 867862500, time_zone(0)).unwrap()),
                ..Default::default()
            }]),
            ..Default::default()
        },
        Case {
            name: "wine-zeroed.zip.bz2",
            expected_encoding: Some(Encoding::Utf8),
            files: Files::NumFiles(11372),
            ..Default::default()
        },
        Case {
            name: "info-zip-unix-extra.zip",
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "bun-darwin-x64/",
                ..Default::default()
            }]),
            ..Default::default()
        },
        Case {
            name: "readme.trailingzip",
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "README",
                ..Default::default()
            }]),
            ..Default::default()
        },
        #[cfg(feature = "lzma")]
        Case {
            name: "found-me-lzma.zip",
            expected_encoding: Some(Encoding::Utf8),
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "found-me.txt",
                content: FileContent::Bytes("Oh no, you found me\n".repeat(5000).into()),
                modified: Some(date((2024, 1, 26), (16, 14, 35), 46003100, time_zone(0)).unwrap()),
                ..Default::default()
            }]),
            ..Default::default()
        },
        #[cfg(feature = "deflate64")]
        Case {
            name: "found-me-deflate64.zip",
            expected_encoding: Some(Encoding::Utf8),
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "found-me.txt",
                content: FileContent::Bytes("Oh no, you found me\n".repeat(5000).into()),
                modified: Some(date((2024, 1, 26), (16, 14, 35), 46003100, time_zone(0)).unwrap()),
                ..Default::default()
            }]),
            ..Default::default()
        },
        // same with bzip2
        #[cfg(feature = "bzip2")]
        Case {
            name: "found-me-bzip2.zip",
            expected_encoding: Some(Encoding::Utf8),
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "found-me.txt",
                content: FileContent::Bytes("Oh no, you found me\n".repeat(5000).into()),
                modified: Some(date((2024, 1, 26), (16, 14, 35), 46003100, time_zone(0)).unwrap()),
                ..Default::default()
            }]),
            ..Default::default()
        },
        // same with zstd
        #[cfg(feature = "zstd")]
        Case {
            name: "found-me-zstd.zip",
            expected_encoding: Some(Encoding::Utf8),
            files: Files::ExhaustiveList(vec![CaseFile {
                name: "found-me.txt",
                content: FileContent::Bytes("Oh no, you found me\n".repeat(5000).into()),
                modified: Some(date((2024, 1, 31), (6, 10, 25), 800491400, time_zone(0)).unwrap()),
                ..Default::default()
            }]),
            ..Default::default()
        },
    ]
}

pub fn streaming_test_cases() -> Vec<Case> {
    vec![
        Case {
            name: "meta.zip",
            files: Files::NumFiles(0),
            ..Default::default()
        },
        Case {
            name: "info-zip-unix-extra.zip",
            files: Files::NumFiles(0),
            ..Default::default()
        },
    ]
}

pub fn check_case(case: &Case, archive: Result<&Archive, &Error>) {
    let case_bytes = case.bytes();

    if let Some(expected) = &case.error {
        let actual = match archive {
            Err(e) => e,
            Ok(_) => panic!("should have failed"),
        };
        let expected = format!("{:#?}", expected);
        let actual = format!("{:#?}", actual);
        assert_eq!(expected, actual);
        return;
    }
    let archive = archive.unwrap_or_else(|e| {
        panic!(
            "{} should have succeeded, but instead: {e:?} ({e})",
            case.name
        )
    });

    assert_eq!(case_bytes.len() as u64, archive.size());

    if let Some(expected) = case.comment {
        assert_eq!(expected, archive.comment())
    }

    if let Some(exp_encoding) = case.expected_encoding {
        assert_eq!(archive.encoding(), exp_encoding);
    }

    assert_eq!(
        case.files.len(),
        archive.entries().count(),
        "{} should have {} entries files",
        case.name,
        case.files.len()
    );

    // then each implementation should check individual files
}

pub fn check_file_against(file: &CaseFile, entry: &Entry, actual_bytes: &[u8]) {
    if let Some(expected) = file.modified {
        assert_eq!(
            expected, entry.modified,
            "entry {} should have modified = {:?}",
            entry.name, expected
        )
    }

    if let Some(mode) = file.mode {
        assert_eq!(entry.mode.0 & 0o777, mode);
    }

    // I have honestly yet to see a zip file _entry_ with a comment.
    assert!(entry.comment.is_empty());

    match entry.kind() {
        EntryKind::File => {
            match &file.content {
                FileContent::Unchecked => {
                    // ah well
                }
                FileContent::Bytes(expected_bytes) => {
                    // first check length
                    assert_eq!(actual_bytes.len(), expected_bytes.len());
                    assert_eq!(actual_bytes, &expected_bytes[..])
                }
                FileContent::File(file_path) => {
                    let expected_bytes = std::fs::read(zips_dir().join(file_path)).unwrap();
                    // first check length
                    assert_eq!(actual_bytes.len(), expected_bytes.len());
                    assert_eq!(actual_bytes, &expected_bytes[..])
                }
            }
        }
        EntryKind::Symlink | EntryKind::Directory => {
            assert!(matches!(file.content, FileContent::Unchecked));
        }
    }
}

// This test subscriber is used to suppress trace-level logs (yet executes
// the code, for coverage reasons)
pub fn install_test_subscriber() {
    let env_filter = tracing_subscriber::EnvFilter::builder().from_env_lossy();
    let sub = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .with_test_writer()
        .finish();
    let sub = DebugOnlySubscriber { inner: sub };
    // fails when called multiple times from the same process (like in `cargo test`), so ignore
    // errors
    let _ = tracing::subscriber::set_global_default(sub);
}

struct DebugOnlySubscriber<S> {
    inner: S,
}

impl<S> tracing::Subscriber for DebugOnlySubscriber<S>
where
    S: tracing::Subscriber,
{
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
        self.inner.new_span(span)
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        self.inner.record(span, values)
    }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
        self.inner.record_follows_from(span, follows)
    }

    fn event(&self, event: &tracing::Event<'_>) {
        if *event.metadata().level() == tracing::Level::TRACE {
            return;
        }

        self.inner.event(event)
    }

    fn enter(&self, span: &span::Id) {
        self.inner.enter(span)
    }

    fn exit(&self, span: &span::Id) {
        self.inner.exit(span)
    }
}
