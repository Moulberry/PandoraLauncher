extern crate wfd;

use wfd::{DialogParams, FOS_ALLOWMULTISELECT};

fn main() {
    let params = DialogParams {
        options: FOS_ALLOWMULTISELECT,
        title: "Select multiple files to open",
        ..Default::default()
    };

    let result = wfd::open_dialog(params);
    println!("{:?}", result);
}