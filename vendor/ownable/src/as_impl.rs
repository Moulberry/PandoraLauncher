macro_rules! impl_as {
    ($as:ident, $base:ident) => {
        // Borrow, Deref

        impl<T: $base> Borrow<T> for $as<T> {
            #[inline(always)]
            fn borrow(&self) -> &T {
                &self.0
            }
        }

        impl<T: $base> BorrowMut<T> for $as<T> {
            #[inline(always)]
            fn borrow_mut(&mut self) -> &mut T {
                &mut self.0
            }
        }

        impl<T: $base> Deref for $as<T> {
            type Target = T;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<T: $base> DerefMut for $as<T> {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        // Debug, Display

        impl<T: $base + Debug> Debug for $as<T> {
            #[inline(always)]
            fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl<T: $base + Display> Display for $as<T> {
            #[inline(always)]
            fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
                self.0.fmt(f)
            }
        }

        // Default

        impl<T: $base + Default> Default for $as<T> {
            fn default() -> Self {
                $as(T::default())
            }
        }

        // From

        impl<T: $base> From<T> for $as<T> {
            #[inline(always)]
            fn from(value: T) -> Self {
                $as(value)
            }
        }

        // Hash

        impl<T: $base + Hash> Hash for $as<T> {
            #[inline(always)]
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.hash(state)
            }
        }

        // Eq, PartialEq

        impl<T: $base + PartialEq> PartialEq for $as<T> {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                self.0.eq(&other.0)
            }
        }

        impl<T: $base + Eq> Eq for $as<T> {}

        // Ord, PartialOrd

        impl<T: $base + PartialOrd> PartialOrd for $as<T> {
            #[inline(always)]
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                self.0.partial_cmp(&other.0)
            }

            #[inline(always)]
            fn lt(&self, other: &Self) -> bool {
                self.0.lt(&other.0)
            }

            #[inline(always)]
            fn le(&self, other: &Self) -> bool {
                self.0.le(&other.0)
            }

            #[inline(always)]
            fn gt(&self, other: &Self) -> bool {
                self.0.gt(&other.0)
            }

            #[inline(always)]
            fn ge(&self, other: &Self) -> bool {
                self.0.ge(&other.0)
            }
        }

        impl<T: $base + Ord> Ord for $as<T> {
            #[inline(always)]
            fn cmp(&self, other: &Self) -> Ordering {
                self.0.cmp(&other.0)
            }
        }
    };
}

pub(crate) use impl_as;
