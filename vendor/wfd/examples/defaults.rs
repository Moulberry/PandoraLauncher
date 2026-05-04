extern crate wfd;

fn main() {
    let result = wfd::open_dialog(Default::default());
    println!("{:?}", result);

    let result = wfd::save_dialog(Default::default());
    println!("{:?}", result);
}