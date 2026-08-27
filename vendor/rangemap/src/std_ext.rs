use core::ops::{Add, Range, RangeInclusive, Sub};

pub trait RangeExt<T> {
    fn overlaps(&self, other: &Self) -> bool;
    fn touches(&self, other: &Self) -> bool;
}

impl<T> RangeExt<T> for Range<T>
where
    T: Ord,
{
    fn overlaps(&self, other: &Self) -> bool {
        use core::cmp::{max, min};
        // Strictly less than, because ends are excluded.
        max(&self.start, &other.start) < min(&self.end, &other.end)
    }

    fn touches(&self, other: &Self) -> bool {
        use core::cmp::{max, min};
        // Less-than-or-equal-to because if one end is excluded, the other is included.
        // I.e. the two could be joined into a single range, because they're overlapping
        // or immediately adjacent.
        max(&self.start, &other.start) <= min(&self.end, &other.end)
    }
}

pub trait RangeInclusiveExt<T> {
    fn overlaps(&self, other: &Self) -> bool;
    fn touches<StepFnsT>(&self, other: &Self) -> bool
    where
        StepFnsT: StepFns<T>;
}

impl<T> RangeInclusiveExt<T> for RangeInclusive<T>
where
    T: Ord + Clone,
{
    fn overlaps(&self, other: &Self) -> bool {
        use core::cmp::{max, min};
        // Less than or equal, because ends are included.
        max(self.start(), other.start()) <= min(self.end(), other.end())
    }

    fn touches<StepFnsT>(&self, other: &Self) -> bool
    where
        StepFnsT: StepFns<T>,
    {
        use core::cmp::{max, min};

        // Touching for end-inclusive ranges is equivalent to touching of
        // slightly longer end-inclusive ranges.
        //
        // We need to do a small dance to avoid arithmetic overflow
        // at the extremes of the key space. And to do this without
        // needing to bound our key type on something like `num::Bounded`
        // (https://docs.rs/num/0.3.0/num/trait.Bounded.html),
        // we'll just extend the end of the _earlier_ range iff
        // its end is already earlier than the latter range's start.
        let max_start = max(self.start(), other.start());
        let min_range_end = min(self.end(), other.end());
        let min_range_end_extended = if min_range_end < max_start {
            StepFnsT::add_one(min_range_end)
        } else {
            min_range_end.clone()
        };
        *max_start <= min_range_end_extended
    }
}

/// Minimal version of unstable [`Step`](core::iter::Step) trait
/// from the Rust standard library.
///
/// This is needed for [`RangeInclusiveMap`](crate::RangeInclusiveMap)
/// because ranges stored as its keys interact with each other
/// when the start of one is _adjacent_ the end of another.
/// I.e. we need a concept of successor values rather than just
/// equality, and that is what `Step` will
/// eventually provide once it is stabilized.
///
/// **NOTE:** This will likely be deprecated and then eventually
/// removed once the standard library's `Step`
/// trait is stabilised, as most crates will then likely implement `Step`
/// for their types where appropriate.
///
/// See [this issue](https://github.com/rust-lang/rust/issues/42168)
/// for details about that stabilization process.
pub trait StepLite {
    /// Returns the _successor_ of `self`.
    ///
    /// If this would overflow the range of values supported by `Self`,
    /// this function is allowed to panic, wrap, or saturate.
    /// The suggested behavior is to panic when debug assertions are enabled,
    /// and to wrap or saturate otherwise.
    fn add_one(&self) -> Self;

    /// Returns the _predecessor_ of `self`.
    ///
    /// If this would overflow the range of values supported by `Self`,
    /// this function is allowed to panic, wrap, or saturate.
    /// The suggested behavior is to panic when debug assertions are enabled,
    /// and to wrap or saturate otherwise.
    fn sub_one(&self) -> Self;
}

// Implement for all common integer types.
macro_rules! impl_step_lite {
    ($($t:ty)*) => ($(
        impl StepLite for $t {
            #[inline]
            fn add_one(&self) -> Self {
                Add::add(*self, 1)
            }

            #[inline]
            fn sub_one(&self) -> Self {
                Sub::sub(*self, 1)
            }
        }
    )*)
}

impl_step_lite!(usize u8 u16 u32 u64 u128 i8 i16 i32 i64 i128);

// The successor of a float is the next representable float,
// i.e. one ULP (unit in the last place) away.
//
// We can't use the standard library's `next_up` and `next_down` for this
// because they were only stabilised in Rust 1.86, well beyond our MSRV,
// so we reimplement them here. These mirror the standard library's versions
// exactly, including their treatment of zeroes (`-0.0` and `0.0` share the
// same neighbours, which agrees with how `NotNan` orders them) and their
// saturation at the infinities. `StepLite` explicitly allows saturating
// at the ends of the range.
//
// We don't need the standard library's NaN check, because `NotNan` has
// already ruled that out for us.
//
// TODO: Delete all of this the next time we raise our MSRV past 1.86,
// and call `next_up`/`next_down` directly instead.
#[cfg(feature = "ordered-float5")]
macro_rules! impl_step_lite_not_nan {
    ($($t:ty => $bits:ty),* $(,)?) => ($(
        impl StepLite for ordered_float::NotNan<$t> {
            #[inline]
            fn add_one(&self) -> Self {
                const SIGN_MASK: $bits = 1 << (<$bits>::BITS - 1);
                // Smallest positive subnormal.
                const TINY_BITS: $bits = 1;

                let bits = self.into_inner().to_bits();
                let next_bits = if bits == <$t>::INFINITY.to_bits() {
                    bits
                } else {
                    let abs = bits & !SIGN_MASK;
                    if abs == 0 {
                        TINY_BITS
                    } else if bits == abs {
                        bits + 1
                    } else {
                        bits - 1
                    }
                };

                // The successor of a non-NaN float is never NaN.
                ordered_float::NotNan::new(<$t>::from_bits(next_bits))
                    .expect("successor of a non-NaN float is never NaN")
            }

            #[inline]
            fn sub_one(&self) -> Self {
                const SIGN_MASK: $bits = 1 << (<$bits>::BITS - 1);
                // Smallest negative subnormal.
                const NEG_TINY_BITS: $bits = (1 << (<$bits>::BITS - 1)) | 1;

                let bits = self.into_inner().to_bits();
                let next_bits = if bits == <$t>::NEG_INFINITY.to_bits() {
                    bits
                } else {
                    let abs = bits & !SIGN_MASK;
                    if abs == 0 {
                        NEG_TINY_BITS
                    } else if bits == abs {
                        bits - 1
                    } else {
                        bits + 1
                    }
                };

                // The predecessor of a non-NaN float is never NaN.
                ordered_float::NotNan::new(<$t>::from_bits(next_bits))
                    .expect("predecessor of a non-NaN float is never NaN")
            }
        }
    )*)
}

#[cfg(feature = "ordered-float5")]
impl_step_lite_not_nan!(f32 => u32, f64 => u64);

// TODO: When on nightly, a blanket implementation for
// all types that implement `core::iter::Step` instead
// of the auto-impl above.

/// Successor and predecessor functions defined for `T`,
/// but as free functions rather than methods on `T` itself.
///
/// This is useful as a workaround for Rust's "orphan rules",
/// which prevent you from implementing [`StepLite`](crate::StepLite) for `T` if `T`
/// is a foreign type.
///
/// **NOTE:** This will likely be deprecated and then eventually
/// removed once the standard library's [`Step`](core::iter::Step)
/// trait is stabilised, as most crates will then likely implement `Step`
/// for their types where appropriate.
///
/// See [this issue](https://github.com/rust-lang/rust/issues/42168)
/// for details about that stabilization process.
///
/// There is also a blanket implementation of `StepFns` for all
/// types implementing `StepLite`. Consumers of this crate should
/// prefer to implement `StepLite` for their own types, and only
/// fall back to `StepFns` when dealing with foreign types.
pub trait StepFns<T> {
    /// Returns the _successor_ of value `start`.
    ///
    /// If this would overflow the range of values supported by `Self`,
    /// this function is allowed to panic, wrap, or saturate.
    /// The suggested behavior is to panic when debug assertions are enabled,
    /// and to wrap or saturate otherwise.
    fn add_one(start: &T) -> T;

    /// Returns the _predecessor_ of value `start`.
    ///
    /// If this would overflow the range of values supported by `Self`,
    /// this function is allowed to panic, wrap, or saturate.
    /// The suggested behavior is to panic when debug assertions are enabled,
    /// and to wrap or saturate otherwise.
    fn sub_one(start: &T) -> T;
}

impl<T> StepFns<T> for T
where
    T: StepLite,
{
    fn add_one(start: &T) -> T {
        start.add_one()
    }

    fn sub_one(start: &T) -> T {
        start.sub_one()
    }
}

#[cfg(all(test, feature = "ordered-float5"))]
mod tests {
    use super::*;
    use alloc as std;
    use alloc::format;
    use ordered_float::NotNan;
    use proptest::prelude::*;
    use test_strategy::proptest;

    fn not_nan(x: f64) -> NotNan<f64> {
        NotNan::new(x).unwrap()
    }

    fn not_nan_32(x: f32) -> NotNan<f32> {
        NotNan::new(x).unwrap()
    }

    // We can't check any of these against the standard library's `next_up`
    // and `next_down`, because those are newer than our MSRV, so spell out
    // what "one ULP away" means in terms of the underlying bit patterns
    // instead.

    #[test]
    fn steps_to_the_next_representable_float() {
        assert_eq!(
            not_nan(1.0).add_one(),
            not_nan(f64::from_bits(1.0f64.to_bits() + 1))
        );
        assert_eq!(
            not_nan(1.0).sub_one(),
            not_nan(f64::from_bits(1.0f64.to_bits() - 1))
        );

        // Negative values run the other way through the bit patterns.
        assert_eq!(
            not_nan(-1.0).add_one(),
            not_nan(f64::from_bits((-1.0f64).to_bits() - 1))
        );
        assert_eq!(
            not_nan(-1.0).sub_one(),
            not_nan(f64::from_bits((-1.0f64).to_bits() + 1))
        );
    }

    #[test]
    fn steps_from_zero_to_the_smallest_subnormals() {
        let tiny = f64::from_bits(1);
        assert_eq!(not_nan(0.0).add_one(), not_nan(tiny));
        assert_eq!(not_nan(0.0).sub_one(), not_nan(-tiny));

        // `NotNan` considers `-0.0` and `0.0` equal, so they have to step
        // to the same neighbours as each other.
        assert_eq!(not_nan(-0.0).add_one(), not_nan(tiny));
        assert_eq!(not_nan(-0.0).sub_one(), not_nan(-tiny));
    }

    #[test]
    fn steps_saturate_at_the_infinities() {
        assert_eq!(not_nan(f64::MAX).add_one(), not_nan(f64::INFINITY));
        assert_eq!(not_nan(f64::INFINITY).add_one(), not_nan(f64::INFINITY));
        assert_eq!(not_nan(f64::MIN).sub_one(), not_nan(f64::NEG_INFINITY));
        assert_eq!(
            not_nan(f64::NEG_INFINITY).sub_one(),
            not_nan(f64::NEG_INFINITY)
        );

        // The infinities still step back inwards, though.
        assert_eq!(not_nan(f64::INFINITY).sub_one(), not_nan(f64::MAX));
        assert_eq!(not_nan(f64::NEG_INFINITY).add_one(), not_nan(f64::MIN));
    }

    #[test]
    fn f32_steps_the_same_way_as_f64() {
        assert_eq!(
            not_nan_32(1.0).add_one(),
            not_nan_32(f32::from_bits(1.0f32.to_bits() + 1))
        );
        assert_eq!(not_nan_32(0.0).add_one(), not_nan_32(f32::from_bits(1)));
        assert_eq!(not_nan_32(-0.0).add_one(), not_nan_32(f32::from_bits(1)));
        assert_eq!(not_nan_32(f32::MAX).add_one(), not_nan_32(f32::INFINITY));
        assert_eq!(
            not_nan_32(f32::INFINITY).add_one(),
            not_nan_32(f32::INFINITY)
        );
    }

    #[proptest]
    fn steps_are_reversible(x: NotNan<f64>) {
        // Not at the infinities, where stepping outwards saturates.
        prop_assume!(x.into_inner().is_finite());
        assert_eq!(x.add_one().sub_one(), x);
        assert_eq!(x.sub_one().add_one(), x);
    }

    #[proptest]
    fn steps_move_in_the_right_direction(x: NotNan<f64>) {
        prop_assume!(x.into_inner().is_finite());
        assert!(x.add_one() > x);
        assert!(x.sub_one() < x);
    }
}
