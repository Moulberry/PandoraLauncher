extern crate async_tar;

use async_std::fs::File;
use async_tar::Builder;

fn main() {
    async_std::task::block_on(async {
        let file = File::create("foo.tar").await.unwrap();
        let mut a = Builder::new(file);

        a.append_path("README.md").await.unwrap();
        a.append_file("lib.rs", &mut File::open("src/lib.rs").await.unwrap())
            .await
            .unwrap();
    });
}
