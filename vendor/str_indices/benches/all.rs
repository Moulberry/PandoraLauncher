#![allow(clippy::uninlined_format_args)]
use std::{fs, path::Path};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use str_indices::{chars, lines, lines_crlf, lines_lf, utf16};

fn all(c: &mut Criterion) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/text");
    let read_text =
        |name: &str| fs::read_to_string(root.join(name)).expect("cannot find benchmark text at");
    // Load benchmark strings.
    let test_strings = vec![
        ("en_0001", "E".into()),
        ("en_0010", read_text("en_10.txt")),
        ("en_0100", read_text("en_100.txt")),
        ("en_1000", read_text("en_1000.txt")),
        ("en_10000", read_text("en_1000.txt").repeat(10)),
        ("jp_0003", "日".into()),
        ("jp_0102", read_text("jp_102.txt")),
        ("jp_1001", read_text("jp_1001.txt")),
        ("jp_10000", read_text("jp_1001.txt").repeat(10)),
    ];

    let line_strings = vec![
        ("lines_100", read_text("lines.txt")),
        ("lines_1000", read_text("lines.txt").repeat(10)),
        ("lines_10000", read_text("lines.txt").repeat(100)),
    ];

    //---------------------------------------------------------
    // Chars.

    // chars::count()
    {
        let mut group = c.benchmark_group("chars::count");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                bench.iter(|| {
                    black_box(chars::count(text));
                })
            });
        }
    }
    {
        // Equivalent implementations using stdlib functions,
        // for performance comparisons.
        let mut group = c.benchmark_group("chars::count_std");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                bench.iter(|| {
                    black_box(text.chars().count());
                })
            });
        }
    }

    // chars::from_byte_idx()
    {
        let mut group = c.benchmark_group("chars::from_byte_idx");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                let idx = text.len();
                bench.iter(|| {
                    black_box(chars::from_byte_idx(text, idx));
                })
            });
        }
    }
    {
        // Equivalent implementations using stdlib functions,
        // for performance comparisons.
        let mut group = c.benchmark_group("chars::from_byte_idx_std");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(format!("std::{}", text_name), |bench| {
                let idx = text.len();
                bench.iter(|| {
                    black_box({
                        let mut byte_idx = idx;
                        // Find the beginning of the code point.
                        while !text.is_char_boundary(byte_idx) {
                            byte_idx -= 1;
                        }
                        // Count the number of chars until the
                        // char that begins at `byte_idx`.
                        text[..byte_idx].chars().count()
                    })
                })
            });
        }
    }

    // chars::to_byte_idx()
    {
        let mut group = c.benchmark_group("chars::to_byte_idx");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                let idx = chars::count(text);
                bench.iter(|| {
                    black_box(chars::to_byte_idx(text, idx));
                })
            });
        }
    }
    {
        // Equivalent implementations using stdlib functions,
        // for performance comparisons.
        let mut group = c.benchmark_group("chars::to_byte_idx_std");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(format!("std::{}", text_name), |bench| {
                let idx = chars::count(text) - 1; // Minus 1 so we can unwrap below.
                bench.iter(|| {
                    black_box(text.char_indices().nth(idx).unwrap().0);
                })
            });
        }
    }

    //---------------------------------------------------------
    // UTF16.

    // utf16::count()
    {
        let mut group = c.benchmark_group("utf16::count");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                bench.iter(|| {
                    black_box(utf16::count(text));
                })
            });
        }
    }

    // utf16::count_surrogates()
    {
        let mut group = c.benchmark_group("utf16::count_surrogates");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                bench.iter(|| {
                    black_box(utf16::count_surrogates(text));
                })
            });
        }
    }

    // utf16::from_byte_idx()
    {
        let mut group = c.benchmark_group("utf16::from_byte_idx");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                let idx = text.len();
                bench.iter(|| {
                    black_box(utf16::from_byte_idx(text, idx));
                })
            });
        }
    }

    // utf16::to_byte_idx()
    {
        let mut group = c.benchmark_group("utf16::to_byte_idx");
        for (text_name, text) in test_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                let idx = utf16::count(text);
                bench.iter(|| {
                    black_box(utf16::to_byte_idx(text, idx));
                })
            });
        }
    }

    //---------------------------------------------------------
    // Lines (unicode).
    let unicode_line_breaks = [
        ("LF", "\u{000A}"),
        ("VT", "\u{000B}"),
        ("FF", "\u{000C}"),
        ("CR", "\u{000D}"),
        ("NEL", "\u{0085}"),
        ("LS", "\u{2028}"),
        ("PS", "\u{2029}"),
        ("CRLF", "\u{000D}\u{000A}"),
    ];

    // lines::count_breaks()
    {
        let mut group = c.benchmark_group("lines::count_breaks");
        for (text_name, text) in line_strings.iter() {
            for (break_name, line_break) in unicode_line_breaks {
                let text = text.replace('\n', line_break);
                group.throughput(Throughput::Bytes(text.len() as u64));
                group.bench_function(format!("{text_name}_{break_name}"), |bench| {
                    bench.iter(|| {
                        black_box(lines::count_breaks(&text));
                    })
                });
            }
        }
    }

    // lines::from_byte_idx()
    {
        let mut group = c.benchmark_group("lines::from_byte_idx");
        for (text_name, text) in line_strings.iter() {
            for (break_name, line_break) in unicode_line_breaks {
                let text = text.replace('\n', line_break);
                group.throughput(Throughput::Bytes(text.len() as u64));
                group.bench_function(format!("{text_name}_{break_name}"), |bench| {
                    let idx = text.len();
                    bench.iter(|| {
                        black_box(lines::from_byte_idx(&text, idx));
                    })
                });
            }
        }
    }

    // lines::to_byte_idx()
    {
        let mut group = c.benchmark_group("lines::to_byte_idx");
        for (text_name, text) in line_strings.iter() {
            for (break_name, line_break) in unicode_line_breaks {
                let text = &text.replace('\n', line_break);
                group.throughput(Throughput::Bytes(text.len() as u64));
                group.bench_function(format!("{text_name}_{break_name}"), |bench| {
                    let idx = lines::count_breaks(text) + 1;
                    bench.iter(|| {
                        black_box(lines::to_byte_idx(text, idx));
                    })
                });
            }
        }
    }

    //---------------------------------------------------------
    // Lines (LF).

    // lines_lf::count_breaks()
    {
        let mut group = c.benchmark_group("lines_lf::count_breaks");
        for (text_name, text) in line_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                bench.iter(|| {
                    black_box(lines_lf::count_breaks(text));
                })
            });
        }
    }
    {
        // Version implemented with stdlib functions,
        // for performance comparisons.  Note: this
        // isn't exactly identical in behavior, since
        // stdlib ignores document-final line breaks.
        // But it should be close enough for perf
        // comparisons.
        let mut group = c.benchmark_group("lines_lf::count_breaks_std");
        for (text_name, text) in line_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                bench.iter(|| {
                    black_box(text.lines().count());
                })
            });
        }
    }

    // lines_lf::from_byte_idx()
    {
        let mut group = c.benchmark_group("lines_lf::from_byte_idx");
        for (text_name, text) in line_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                let idx = text.len();
                bench.iter(|| {
                    black_box(lines_lf::from_byte_idx(text, idx));
                })
            });
        }
    }

    // lines_lf::to_byte_idx()
    {
        let mut group = c.benchmark_group("lines_lf::to_byte_idx");
        for (text_name, text) in line_strings.iter() {
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(*text_name, |bench| {
                let idx = lines_lf::count_breaks(text) + 1;
                bench.iter(|| {
                    black_box(lines_lf::to_byte_idx(text, idx));
                })
            });
        }
    }

    //---------------------------------------------------------
    // Lines (CRLF).
    let crlf_line_breaks = [
        ("LF", "\u{000A}"),
        ("CR", "\u{000D}"),
        ("CRLF", "\u{000D}\u{000A}"),
    ];

    // lines_crlf::count_breaks()
    {
        let mut group = c.benchmark_group("lines_crlf::count_breaks");
        for (text_name, text) in line_strings.iter() {
            for (break_name, line_break) in crlf_line_breaks {
                let text = &text.replace('\n', line_break);
                group.throughput(Throughput::Bytes(text.len() as u64));
                group.bench_function(format!("{text_name}_{break_name}"), |bench| {
                    bench.iter(|| {
                        black_box(lines_crlf::count_breaks(text));
                    })
                });
            }
        }
    }

    // lines_crlf::from_byte_idx()
    {
        let mut group = c.benchmark_group("lines_crlf::from_byte_idx");
        for (text_name, text) in line_strings.iter() {
            for (break_name, line_break) in crlf_line_breaks {
                let text = &text.replace('\n', line_break);
                group.throughput(Throughput::Bytes(text.len() as u64));
                group.bench_function(format!("{text_name}_{break_name}"), |bench| {
                    let idx = text.len();
                    bench.iter(|| {
                        black_box(lines_crlf::from_byte_idx(text, idx));
                    })
                });
            }
        }
    }

    // lines_crlf::to_byte_idx()
    {
        let mut group = c.benchmark_group("lines_crlf::to_byte_idx");
        for (text_name, text) in line_strings.iter() {
            for (break_name, line_break) in crlf_line_breaks {
                let text = &text.replace('\n', line_break);
                group.throughput(Throughput::Bytes(text.len() as u64));
                group.bench_function(format!("{text_name}_{break_name}"), |bench| {
                    let idx = lines_crlf::count_breaks(text) + 1;
                    bench.iter(|| {
                        black_box(lines_crlf::to_byte_idx(text, idx));
                    })
                });
            }
        }
    }
}

//-------------------------------------------------------------

criterion_group!(benches, all,);
criterion_main!(benches);
