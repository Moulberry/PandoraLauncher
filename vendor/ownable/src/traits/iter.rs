use crate::traits::{IntoOwned, ToBorrowed, ToOwned};
use alloc::collections::{BTreeSet, BinaryHeap, LinkedList, VecDeque};
use alloc::vec::Vec;

// Macro for things to be iterated
macro_rules! iter_impl {
    ($ty:ident, $($extra:tt)?) => {
        impl<'a, T> ToBorrowed<'a> for $ty<T>
        where
            T: ToBorrowed<'a> $(+ $extra)?,
        {
            #[inline]
            fn to_borrowed(&'a self) -> Self {
                self.iter().map(ToBorrowed::to_borrowed).collect()
            }
        }

        impl<T> ToOwned for $ty<T>
        where
            T: ToOwned,
          $(  <T as ToOwned>::Owned: $extra,)?
        {
            type Owned = $ty<T::Owned>;

            #[inline]
            fn to_owned(&self) -> Self::Owned {
                self.iter().map(ToOwned::to_owned).collect()
            }
        }

        impl<T> IntoOwned for $ty<T>
        where
            T: IntoOwned,
          $(  <T as IntoOwned>::Owned: $extra,)?
        {
            type Owned = $ty<T::Owned>;

            #[inline]
            fn into_owned(self) -> Self::Owned {
                self.into_iter().map(IntoOwned::into_owned).collect()
            }
        }
    };
}

iter_impl!(Vec,);
iter_impl!(VecDeque,);
iter_impl!(LinkedList,);
iter_impl!(BinaryHeap, Ord);
iter_impl!(BTreeSet, Ord);
