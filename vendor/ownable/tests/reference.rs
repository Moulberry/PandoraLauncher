use ownable::{IntoOwned, ToBorrowed, ToOwned};
use std::borrow::Cow;

#[derive(Debug, PartialEq, ToBorrowed, ToOwned, IntoOwned)]
#[ownable(reference = "'t")]
struct Test<'t, 'u, T: ?Sized, U: ?Sized + 'u>
where
    T: core::fmt::Debug,
    U: ToOwned,
    <U as std::borrow::ToOwned>::Owned: core::fmt::Debug,
{
    y: &'t T,
    z: Cow<'u, U>,
    nested: Option<Box<Test<'t, 'u, T, U>>>,
}

#[derive(Debug, PartialEq, ToBorrowed, ToOwned, IntoOwned)]
#[ownable(reference = "'t")]
struct Test2<'t, 'u, 'v, T: ?Sized, U: ?Sized + 'u, V>
where
    T: core::fmt::Debug,
    U: ToOwned,
    <U as std::borrow::ToOwned>::Owned: core::fmt::Debug,
    V: ?Sized + ToOwned + 'v,
    <V as std::borrow::ToOwned>::Owned: core::fmt::Debug,
{
    v: Cow<'v, V>,
    test: Test<'t, 'u, T, U>,
}

#[test]
fn test() {
    let value: String = "value".to_string();
    let v0: Test2<'_, '_, '_, str, str, str> = Test2 {
        v: Cow::Borrowed("v"),
        test: Test {
            y: "lnk",
            z: Cow::Borrowed(&value),
            nested: None,
        },
    };
    let v1: Test2<'_, '_, '_, str, str, str> = v0.to_borrowed();
    assert_eq!(v0, v1);
    let v2: Test2<'_, 'static, 'static, str, str, str> = v0.to_owned();
    assert_eq!(v0, v2);
    let v3: Test2<'_, 'static, 'static, str, str, str> = v0.into_owned();
    assert_eq!(v2, v3);
}
