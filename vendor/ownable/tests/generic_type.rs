use ownable::{IntoOwned, ToBorrowed, ToOwned};
use std::borrow::Cow;

#[derive(Debug, PartialEq, IntoOwned, ToOwned, ToBorrowed)]
struct Test<'t, T: ?Sized + 't>
where
    T: ToOwned,
    <T as std::borrow::ToOwned>::Owned: core::fmt::Debug,
{
    cow: Cow<'t, T>,
}

#[test]
fn test() {
    let value: String = "value".to_string();
    let v0: Test<'_, str> = Test {
        cow: Cow::Borrowed(&value),
    };
    let v1: Test<'_, str> = v0.to_borrowed();
    assert_eq!(v0, v1);
    let v2: Test<'static, str> = v0.to_owned();
    assert_eq!(v0, v2);
    let v3: Test<'static, str> = v0.into_owned();
    assert_eq!(v2, v3);
}
