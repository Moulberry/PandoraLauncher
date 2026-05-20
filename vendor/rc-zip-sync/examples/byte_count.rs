use std::{ffi::OsString, fmt, fs::File, io::Read, sync::Mutex, thread, time};

use rc_zip_sync::{ArchiveHandle, EntryHandle, ReadZip};

/// Display counts for each byte in a zip's entries
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let zip_path = parse_args()?;
    let zip_file = File::open(&zip_path)?;
    let archive = zip_file.read_zip()?;

    let start = time::Instant::now();
    let counts = byte_count_multi_threaded(&archive);
    print_stats(counts, start.elapsed());

    Ok(())
}

fn parse_args() -> Result<OsString, &'static str> {
    const USAGE: &str = "Usage: cargo run -r --example=byte_count -- [ZIP_FILE]";
    let mut args = std::env::args_os();
    let _bin = args.next();
    let (Some(zip_path), None) = (args.next(), args.next()) else {
        return Err(USAGE);
    };
    if zip_path
        .to_str()
        .is_some_and(|path| ["help", "--help", "-h"].contains(&path))
    {
        return Err(USAGE);
    }

    Ok(zip_path)
}

/// A count stored for each byte value where the index _is_ the byte value
struct Counts([u64; 256]);

impl Counts {
    fn new() -> Self {
        Self([0; _])
    }

    fn inc(&mut self, byte: u8) {
        self.0[usize::from(byte)] += 1;
    }

    fn reduce(&mut self, other: &Self) {
        for (sum, single) in self.0.iter_mut().zip(other.0.iter()) {
            *sum += single;
        }
    }
}

/// Counts the number of each byte (0-255) in a zip's entries splitting the work across multiple
/// threads
///
/// This is your typical map-reduce style problem. Things are _map_ed by creating the set of counts
/// from a single entry in the zip file. Then the work is _reduce_d by combining the counts.
///
/// The work is split across a pool of worker threads where each worker takes turns fetching an
/// entry from the archive to then read over the entry and reduce it down to counts. Then the final
/// counts are totaled together as each worker finishes.
fn byte_count_multi_threaded(archive: &ArchiveHandle<'_, File>) -> Counts {
    let mut total_counts = Counts::new();
    let entries = Mutex::new(archive.entries());
    let num_workers = thread::available_parallelism().unwrap();
    thread::scope(|s| {
        let worker_handles: Vec<_> = (1..num_workers.into())
            .map(|_| s.spawn(|| byte_count_worker(&entries)))
            .collect();
        total_counts = byte_count_worker(&entries);
        for handle in worker_handles {
            let counts = handle.join().unwrap();
            total_counts.reduce(&counts);
        }
    });
    let mut entries = entries.into_inner().unwrap();
    assert!(
        entries.next().is_none(),
        "we should be finished and yet work remains. did all the workers die?"
    );
    total_counts
}

type ZipEntry<'zip> = EntryHandle<'zip, File>;

fn byte_count_worker<'zip>(entries: &Mutex<impl Iterator<Item = ZipEntry<'zip>>>) -> Counts {
    let mut local_counts = Counts::new();
    loop {
        let mut entries = entries.lock().unwrap();
        let Some(entry) = entries.next() else {
            // no more work!
            break;
        };
        // !!IMPORTANT!! dropping the lock before handling the entry otherwise we're effectively
        // single-threaded still :)
        drop(entries);
        if let Err(err) = entry_add_byte_counts(entry, &mut local_counts) {
            eprintln!("error extracting entry: {err}");
        }
    }

    local_counts
}

fn entry_add_byte_counts(entry: ZipEntry<'_>, counts: &mut Counts) -> rc_zip::Result<()> {
    if entry.kind().is_file() {
        let mut buf = [0; 8 * 1024];
        let mut entry_reader = entry.reader();
        while let Ok(num_bytes) = entry_reader.read(&mut buf) {
            if num_bytes == 0 {
                // finished reading!
                break;
            }
            for byte in &buf[..num_bytes] {
                counts.inc(*byte);
            }
        }
    }

    Ok(())
}

struct Byte(u8);

impl fmt::Display for Byte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let byte = self.0;
        if byte.is_ascii_graphic() {
            write!(f, "{:<4}", char::from(byte))
        } else {
            write!(f, "0x{byte:02x}")
        }
    }
}

struct Bar(u64, u64);

impl fmt::Display for Bar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const LENGTH: f32 = 60.0;

        let &Self(count, max) = self;
        let percentage = count as f32 / max as f32;
        let block_length = (LENGTH * percentage).floor() as usize;
        for _ in 0..block_length {
            f.write_str("█")?;
        }

        let block_sections = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
        let index = (((LENGTH * percentage) - block_length as f32) * 8.0).round() as usize;
        f.write_str(block_sections[index])
    }
}

fn print_stats(counts: Counts, took: time::Duration) {
    let counts = counts.0;

    let max = counts.into_iter().max().unwrap();
    let total: u64 = counts.into_iter().sum();
    println!("┏━━━━━━┳━━━━━━━━┳━━━━━━━━━━━┳━━━━━━━━━━━━");
    println!("┃ byte ┃ perc.  ┃   count   ┃ bar");
    println!("┣━━━━━━╋━━━━━━━━╋━━━━━━━━━━━╋━━━━━━━━━━━━");
    for (byte, count) in counts.into_iter().enumerate() {
        let byte = Byte(byte.try_into().unwrap());

        let percent = count as f32 / total as f32 * 100.0;
        let bar = Bar(count, max);
        println!("┃ {byte} ┃ {percent:>5.2}% ┃ {count:>9} ┃ {bar}");
    }
    println!("┗━━━━━━┻━━━━━━━━┻━━━━━━━━━━━┻━━━━━━━━━━━━");
    println!("Counted {total} bytes in {took:.2?}");
}
