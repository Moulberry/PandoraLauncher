use std::fs::File;
use std::num::NonZeroU64;

// cargo run --example reflink_block V:/file.bin V:/file_cow.bin 4096

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().collect();

    let [_, src_file, tgt_file, cluster_size] = &args[..] else {
        eprintln!(
            "Usage: {} <source_file> <target_file> <cluster_size>",
            args[0]
        );
        return Ok(());
    };
    let cluster_size: NonZeroU64 = cluster_size.parse().expect("cannot parse cluster size");

    let from_file = File::open(src_file)?;
    let len = from_file.metadata()?.len();
    let to_file = File::create(tgt_file)?;
    to_file.set_len(len)?;

    let mut offset = 0u64;
    while offset < len {
        let src_length = if cfg!(windows) {
            // Windows API clones the entire cluster regardless of the number of bytes actually used
            // by the file in that cluster.
            cluster_size
        } else {
            cluster_size.min(NonZeroU64::new(len - offset).unwrap())
        };

        println!("reflink {offset}, {src_length}");
        reflink_copy::ReflinkBlockBuilder::new(&from_file, &to_file, src_length)
            .from_offset(offset)
            .to_offset(offset)
            .cluster_size(cluster_size)
            .reflink_block()?;

        offset += src_length.get();
    }
    Ok(())
}
