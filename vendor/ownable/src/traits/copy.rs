use crate::traits::{IntoOwned, ToBorrowed, ToOwned};
use core::num::{NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize};
use core::num::{NonZeroU128, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize};

// ()

impl ToBorrowed<'_> for () {
    #[inline(always)]
    fn to_borrowed(&self) -> Self {}
}

impl ToOwned for () {
    type Owned = ();

    #[inline(always)]
    fn to_owned(&self) -> Self::Owned {}
}

impl IntoOwned for () {
    type Owned = ();

    #[inline(always)]
    fn into_owned(self) -> Self::Owned {}
}

// Copy

macro_rules! copy_impl {
    () => {};
    ($t:ident) => {
        impl ToBorrowed<'_> for $t {
            #[inline(always)]
            fn to_borrowed(&self) -> Self {
                *self
            }
        }
        impl ToOwned for $t {
            type Owned = $t;

            #[inline(always)]
            fn to_owned(&self) -> Self::Owned {
                *self
            }
        }
        impl IntoOwned for $t {
            type Owned = $t;

            #[inline(always)]
            fn into_owned(self) -> Self::Owned {
                self
            }
        }
    };
    ($t:ident, $($y:ident),+) => {
        copy_impl!($t);
        copy_impl!($($y),+);
    };
}

copy_impl!(u8, u16, u32, u64, u128, usize);
copy_impl!(i8, i16, i32, i64, i128, isize);
copy_impl!(f32, f64, bool, char);

copy_impl!(
    NonZeroU8,
    NonZeroU16,
    NonZeroU32,
    NonZeroU64,
    NonZeroU128,
    NonZeroUsize
);
copy_impl!(
    NonZeroI8,
    NonZeroI16,
    NonZeroI32,
    NonZeroI64,
    NonZeroI128,
    NonZeroIsize
);
