//! An example of extracting a file in an archive.
//!
//! Takes a tarball on standard input, looks for an entry with a listed file
//! name as the first argument provided, and then prints the contents of that
//! file to stdout.

extern crate async_tar;

use async_std::{
    io::{copy, stdin, stdout},
    path::Path,
    prelude::*,
};
use std::env::args_os;

use async_tar::Archive;

fn main() {
    async_std::task::block_on(async {
        let first_arg = args_os().nth(1).unwrap();
        let filename = Path::new(&first_arg);
        let ar = Archive::new(stdin());
        let mut entries = ar.entries().unwrap();
        while let Some(file) = entries.next().await {
            let mut f = file.unwrap();
            if f.path().unwrap() == filename {
                copy(&mut f, &mut stdout()).await.unwrap();
            }
        }
    });
}
