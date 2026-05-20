use ownable::{IntoOwned, ToBorrowed, ToOwned};
use std::borrow::Cow;

#[derive(Debug, PartialEq, IntoOwned, ToOwned, ToBorrowed)]
#[ownable(function = false)]
struct Test<'a> {
    cow: Cow<'a, str>,
}

#[test]
fn test() {
    // this would not be used when the functions are generated
    #[deny(unused_imports)]
    use ownable::traits::{IntoOwned, ToBorrowed, ToOwned};

    let value: String = "value".to_string();
    let v0: Test<'_> = Test {
        cow: Cow::Borrowed(&value),
    };
    let v1: Test<'_> = v0.to_borrowed();
    assert_eq!(v0, v1);
    let v2: Test<'static> = v0.to_owned();
    assert_eq!(v0, v2);
    let v3: Test<'static> = v0.into_owned();
    assert_eq!(v2, v3);
}
