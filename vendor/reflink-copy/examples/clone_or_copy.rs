// cargo run --example clone_or_copy V:\file.bin V:\file-clone.bin

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <source_file> <target_file>", args[0]);
        return Ok(());
    }
    let src_file = &args[1];
    let tgt_file = &args[2];

    let result = reflink_copy::reflink_or_copy(src_file, tgt_file)?;
    match result {
        Some(_) => println!("File has been copied"),
        None => println!("File has been cloned"),
    }
    Ok(())
}
