use ownable::{AsClone, AsCopy, IntoOwned, ToBorrowed, ToOwned};
use std::borrow::Cow;
use std::net::Ipv4Addr;

#[derive(Debug, PartialEq, IntoOwned, ToOwned, ToBorrowed)]
struct Test<'a> {
    cow: Cow<'a, str>,
    nested: Option<Box<Test<'a>>>,
    #[ownable(clone)]
    cloned: String,
    copy: usize,
    as_copy: AsCopy<Ipv4Addr>,
    as_clone: AsClone<String>,
    tuple: (Cow<'a, str>, u64),
}

#[test]
fn test() {
    let value: String = "value".to_string();
    let v0: Test<'_> = Test {
        cow: Cow::Borrowed(&value),
        nested: None,
        cloned: "cloned".to_string(),
        copy: 0,
        as_copy: AsCopy(Ipv4Addr::LOCALHOST),
        as_clone: AsClone("as_clone".to_string()),
        tuple: (Cow::Borrowed(&value), 64),
    };
    let v1: Test<'_> = v0.to_borrowed();
    assert_eq!(v0, v1);
    let v2: Test<'static> = v0.to_owned();
    assert_eq!(v0, v2);
    let v3: Test<'static> = v0.into_owned();
    assert_eq!(v2, v3);
}
