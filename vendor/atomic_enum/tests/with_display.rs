use core::sync::atomic::Ordering;
use std::fmt;
use std::fmt::{Display, Formatter};

use atomic_enum::atomic_enum;

#[atomic_enum]
#[derive(PartialEq, Eq)]
enum DisplayableEnum {
    Foo,
}

impl Display for DisplayableEnum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DisplayableEnum::Foo => write!(f, "Foo"),
        }
    }
}

#[test]
fn test_displayable_enum() {
    let e = AtomicDisplayableEnum::new(DisplayableEnum::Foo);
    assert_eq!(format!("{}", e.load(Ordering::SeqCst)), "Foo");
}
