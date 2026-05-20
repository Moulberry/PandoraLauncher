use formatx::formatx;
use std::fmt::Display;

#[derive(Debug)]
struct Foo {
    _bar: String,
}

impl Display for Foo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn main() {
    let foo = Foo {
        _bar: "foo-bar-struct".to_owned(),
    };

    println!("{}", formatx!("{:#?}", foo).unwrap());
}
