extern crate wfd;

use wfd::{DialogParams, FOS_PICKFOLDERS, HWND};

fn main() {
    // Replace this with a real HWND to test
    let hwnd = 0xdeadbeef as HWND;

    let params = DialogParams {
        options: FOS_PICKFOLDERS,
        owner: Some(hwnd),
        title: "Select a directory",
        ..Default::default()
    };

    let result = wfd::open_dialog(params);
    println!("{:?}", result);
}