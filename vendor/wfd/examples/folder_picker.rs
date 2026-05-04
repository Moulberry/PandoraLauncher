extern crate wfd;

use wfd::{DialogParams, FOS_PICKFOLDERS};

fn main() {
    let params = DialogParams {
        options: FOS_PICKFOLDERS,
        title: "Select a directory",
        ..Default::default()
    };

    let result = wfd::open_dialog(params);
    println!("{:?}", result);
}