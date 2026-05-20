use ownable::{IntoOwned, ToBorrowed, ToOwned};
use std::borrow::Cow;

#[derive(Debug, PartialEq, IntoOwned, ToOwned, ToBorrowed)]
struct Test<'t, const N: usize> {
    cow: Cow<'t, [u8; N]>,
}

#[test]
fn test() {
    let value: &[u8; 5] = b"value";
    let v0: Test<'_, 5> = Test {
        cow: Cow::Borrowed(value),
    };
    let v1: Test<'_, 5> = v0.to_borrowed();
    assert_eq!(v0, v1);
    let v2: Test<'static, 5> = v0.to_owned();
    assert_eq!(v0, v2);
    let v3: Test<'static, 5> = v0.into_owned();
    assert_eq!(v2, v3);
}
