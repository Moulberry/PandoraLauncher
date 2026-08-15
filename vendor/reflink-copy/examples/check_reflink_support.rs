// cargo run --example check_reflink_support V:\folder1 X:\folder2

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <source_path> <target_path>", args[0]);
        return Ok(());
    }
    let src_path = &args[1];
    let tgt_path = &args[2];

    let result = reflink_copy::check_reflink_support(src_path, tgt_path)?;
    println!("{result:?}");
    Ok(())
}
