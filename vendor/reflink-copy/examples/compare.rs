use std::fs;
use std::io::{self, Read};
use std::time::Instant;

fn main() {
    let mut base_file = fs::File::create("base.txt").unwrap();
    let mut src = io::repeat(65).take(100 * 1024 * 1024); // 100 MB
    io::copy(&mut src, &mut base_file).unwrap();

    let before_reflink = Instant::now();
    match reflink_copy::reflink("base.txt", "reflinked.txt") {
        Ok(()) => {}
        Err(e) => {
            println!("Error during reflinking:\n{:?}", e);
            fs::remove_file("base.txt").unwrap();
            return;
        }
    };
    println!("Time to reflink: {:?}", Instant::now() - before_reflink);

    let before_copy = Instant::now();
    fs::copy("base.txt", "copied.txt").unwrap();
    println!("Time to copy: {:?}", Instant::now() - before_copy);

    fs::remove_file("base.txt").unwrap();
    fs::remove_file("reflinked.txt").unwrap();
    fs::remove_file("copied.txt").unwrap();
}
