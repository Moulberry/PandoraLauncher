//! A generic range allocator for managing sub-ranges within a larger range.
//!
//! This crate provides [`RangeAllocator`], which hands out non-overlapping
//! `Range<T>` values from a pool. It uses a best-fit strategy to reduce
//! fragmentation and automatically merges adjacent free ranges on deallocation.
//!
//! # Example
//!
//! ```
//! use range_alloc::RangeAllocator;
//!
//! let mut alloc = RangeAllocator::new(0..1024);
//!
//! // Allocate two regions.
//! let a = alloc.allocate_range(256).unwrap(); // 0..256
//! let b = alloc.allocate_range(128).unwrap(); // 256..384
//!
//! // Free the first region so it can be reused.
//! alloc.free_range(a);
//! ```
//!
//! # Minimum Supported Rust Version
//!
//! The MSRV of this crate is at least 1.31, possibly earlier. It will only be
//! bumped in a breaking release.

use std::{
    fmt::Debug,
    iter::Sum,
    ops::{Add, AddAssign, Range, Rem, Sub},
};

/// A best-fit range allocator over a generic index type `T`.
///
/// `RangeAllocator` manages a single contiguous range and hands out
/// non-overlapping sub-ranges on request. Freed ranges are automatically
/// merged with their neighbors.
///
/// # Example
///
/// ```
/// use range_alloc::RangeAllocator;
///
/// let mut alloc = RangeAllocator::new(0..100);
/// let r = alloc.allocate_range(10).unwrap();
/// assert_eq!(r, 0..10);
/// alloc.free_range(r);
/// ```
#[derive(Debug)]
pub struct RangeAllocator<T> {
    /// The range this allocator covers.
    initial_range: Range<T>,
    /// A Vec of ranges in this heap which are unused.
    /// Must be ordered with ascending range start to permit short circuiting allocation.
    /// No two ranges in this vec may overlap.
    free_ranges: Vec<Range<T>>,
}

/// The error returned when an allocation cannot be satisfied.
///
/// Contains the total free space that is available but fragmented
/// across non-contiguous ranges.
#[derive(Clone, Debug, PartialEq)]
pub struct RangeAllocationError<T> {
    /// The total length of all free ranges combined. When this is
    /// greater than or equal to the requested length, the allocation
    /// failed due to fragmentation rather than insufficient space.
    pub fragmented_free_length: T,
}

impl<T> RangeAllocator<T>
where
    T: Clone + Copy + Add<Output = T> + AddAssign + Sub<Output = T> + Eq + PartialOrd + Debug,
{
    /// Creates a new allocator that manages the given range.
    ///
    /// The entire range starts as free and available for allocation.
    ///
    /// # Example
    ///
    /// ```
    /// use range_alloc::RangeAllocator;
    ///
    /// let alloc = RangeAllocator::new(0u32..1024);
    /// assert!(alloc.is_empty());
    /// ```
    pub fn new(range: Range<T>) -> Self {
        RangeAllocator {
            initial_range: range.clone(),
            free_ranges: vec![range],
        }
    }

    /// Returns the full range this allocator was created with
    /// (including any extensions from [`grow_to`](Self::grow_to)).
    pub fn initial_range(&self) -> &Range<T> {
        &self.initial_range
    }

    /// Extends the allocator's range to a new end value.
    ///
    /// The newly added region (`old_end..new_end`) becomes available for
    /// allocation. If the last free range is adjacent to the old end, it
    /// is extended in place rather than creating a new entry.
    ///
    /// # Example
    ///
    /// ```
    /// use range_alloc::RangeAllocator;
    ///
    /// let mut alloc = RangeAllocator::new(0..10);
    /// alloc.allocate_range(10).unwrap();
    /// // Out of space -- grow the pool.
    /// alloc.grow_to(20);
    /// let r = alloc.allocate_range(5).unwrap();
    /// assert_eq!(r, 10..15);
    /// ```
    pub fn grow_to(&mut self, new_end: T) {
        let initial_range_end = self.initial_range.end;
        if let Some(last_range) = self
            .free_ranges
            .last_mut()
            .filter(|last_range| last_range.end == initial_range_end)
        {
            last_range.end = new_end;
        } else {
            self.free_ranges.push(self.initial_range.end..new_end);
        }

        self.initial_range.end = new_end;
    }

    fn allocate_range_impl(
        &mut self,
        length: T,
        align_start: impl Fn(T) -> T,
    ) -> Result<Range<T>, RangeAllocationError<T>> {
        assert_ne!(length + length, length);

        // This is actually correct. With the trait bound as it is, we have
        // no way to summon a value of 0 directly, so we make one by subtracting
        // something from itself. Once the trait bound can be changed, this can
        // be fixed.
        #[allow(clippy::eq_op)]
        let mut fragmented_free_length = length - length;
        let mut best_fit: Option<(usize, T)> = None;

        for (index, range) in self.free_ranges.iter().cloned().enumerate() {
            let range_length = range.end - range.start;
            fragmented_free_length += range_length;

            let aligned_start = align_start(range.start);

            if aligned_start >= range.end {
                continue;
            }
            let usable_length = range.end - aligned_start;
            if usable_length < length {
                continue;
            } else if usable_length == length {
                // Found a perfect fit, so stop looking.
                best_fit = Some((index, aligned_start));
                break;
            }
            best_fit = Some(match best_fit {
                Some((best_index, best_aligned_start)) => {
                    // Find best fit for this allocation to reduce memory fragmentation.
                    let best_usable = self.free_ranges[best_index].end - best_aligned_start;
                    if usable_length < best_usable {
                        (index, aligned_start)
                    } else {
                        (best_index, best_aligned_start)
                    }
                }
                None => (index, aligned_start),
            });
        }

        match best_fit {
            Some((index, aligned_start)) => {
                let range = self.free_ranges[index].clone();
                let alloc_end = aligned_start + length;

                let has_prefix = aligned_start > range.start;
                let has_suffix = alloc_end < range.end;

                match (has_prefix, has_suffix) {
                    (false, false) => {
                        self.free_ranges.remove(index);
                    }
                    (false, true) => {
                        self.free_ranges[index].start = alloc_end;
                    }
                    (true, false) => {
                        self.free_ranges[index].end = aligned_start;
                    }
                    (true, true) => {
                        self.free_ranges[index].end = aligned_start;
                        self.free_ranges.insert(index + 1, alloc_end..range.end);
                    }
                }

                Ok(aligned_start..alloc_end)
            }
            None => Err(RangeAllocationError {
                fragmented_free_length,
            }),
        }
    }

    /// Allocates a sub-range of the given `length`.
    ///
    /// Uses a best-fit strategy: the smallest free range that can satisfy
    /// the request is chosen to minimise fragmentation.
    ///
    /// # Panics
    ///
    /// Panics if `length` is zero.
    ///
    /// # Example
    ///
    /// ```
    /// use range_alloc::RangeAllocator;
    ///
    /// let mut alloc = RangeAllocator::new(0..100);
    /// let a = alloc.allocate_range(30).unwrap();
    /// let b = alloc.allocate_range(20).unwrap();
    /// assert_eq!(a, 0..30);
    /// assert_eq!(b, 30..50);
    /// ```
    pub fn allocate_range(&mut self, length: T) -> Result<Range<T>, RangeAllocationError<T>> {
        self.allocate_range_impl(length, |start| start)
    }

    /// Allocates a sub-range of the given `length` whose start is aligned
    /// to a multiple of `alignment`.
    ///
    /// Any space before the aligned start within a free range is kept free
    /// and available for future allocations -- no space is wasted on
    /// alignment padding.
    ///
    /// Uses the same best-fit strategy as [`allocate_range`](Self::allocate_range).
    ///
    /// # Panics
    ///
    /// Panics if `length` or `alignment` is zero.
    ///
    /// # Example
    ///
    /// ```
    /// use range_alloc::RangeAllocator;
    ///
    /// let mut alloc = RangeAllocator::new(0..256);
    /// // Offset the free region so the next aligned start is not at 0.
    /// alloc.allocate_range(1).unwrap(); // 0..1
    ///
    /// // Allocate 16 units starting at the next multiple of 16.
    /// let r = alloc.allocate_range_aligned(16, 16).unwrap();
    /// assert_eq!(r, 16..32);
    ///
    /// // The gap (1..16) is still free and usable.
    /// let small = alloc.allocate_range(15).unwrap();
    /// assert_eq!(small, 1..16);
    /// ```
    pub fn allocate_range_aligned(
        &mut self,
        length: T,
        alignment: T,
    ) -> Result<Range<T>, RangeAllocationError<T>>
    where
        T: Rem<Output = T>,
    {
        assert_ne!(alignment + alignment, alignment);
        self.allocate_range_impl(length, |start| {
            let padding = (alignment - start % alignment) % alignment;
            start + padding
        })
    }

    /// Returns a previously allocated range to the free pool.
    ///
    /// Adjacent free ranges are automatically merged to reduce
    /// fragmentation.
    ///
    /// # Panics
    ///
    /// Panics if `range` is outside the allocator's initial range, is
    /// empty, or overlaps with an already-free range.
    ///
    /// # Example
    ///
    /// ```
    /// use range_alloc::RangeAllocator;
    ///
    /// let mut alloc = RangeAllocator::new(0..10);
    /// let r = alloc.allocate_range(10).unwrap();
    /// alloc.free_range(r);
    /// assert!(alloc.is_empty());
    /// ```
    pub fn free_range(&mut self, range: Range<T>) {
        assert!(self.initial_range.start <= range.start && range.end <= self.initial_range.end);
        assert!(range.start < range.end);

        // Get insertion position.
        let i = self
            .free_ranges
            .iter()
            .position(|r| r.start > range.start)
            .unwrap_or(self.free_ranges.len());

        // Try merging with neighboring ranges in the free list.
        // Before: |left|-(range)-|right|
        if i > 0 && range.start == self.free_ranges[i - 1].end {
            // Merge with |left|.
            self.free_ranges[i - 1].end =
                if i < self.free_ranges.len() && range.end == self.free_ranges[i].start {
                    // Check for possible merge with |left| and |right|.
                    let right = self.free_ranges.remove(i);
                    right.end
                } else {
                    range.end
                };

            return;
        } else if i < self.free_ranges.len() && range.end == self.free_ranges[i].start {
            // Merge with |right|.
            self.free_ranges[i].start = if i > 0 && range.start == self.free_ranges[i - 1].end {
                // Check for possible merge with |left| and |right|.
                let left = self.free_ranges.remove(i - 1);
                left.start
            } else {
                range.start
            };

            return;
        }

        // Debug checks
        assert!(
            (i == 0 || self.free_ranges[i - 1].end < range.start)
                && (i >= self.free_ranges.len() || range.end < self.free_ranges[i].start)
        );

        self.free_ranges.insert(i, range);
    }

    /// Returns an iterator over all currently allocated (non-free) ranges.
    ///
    /// The ranges are yielded in ascending order.
    ///
    /// # Example
    ///
    /// ```
    /// use range_alloc::RangeAllocator;
    ///
    /// let mut alloc = RangeAllocator::new(0..30);
    /// alloc.allocate_range(10).unwrap(); // 0..10
    /// alloc.allocate_range(10).unwrap(); // 10..20
    ///
    /// // Adjacent allocations appear as a single contiguous range.
    /// let allocated: Vec<_> = alloc.allocated_ranges().collect();
    /// assert_eq!(allocated, vec![0..20]);
    /// ```
    pub fn allocated_ranges(&self) -> impl Iterator<Item = Range<T>> + '_ {
        let first = match self.free_ranges.first() {
            Some(Range { ref start, .. }) if *start > self.initial_range.start => {
                Some(self.initial_range.start..*start)
            }
            None => Some(self.initial_range.clone()),
            _ => None,
        };

        let last = match self.free_ranges.last() {
            Some(Range { end, .. }) if *end < self.initial_range.end => {
                Some(*end..self.initial_range.end)
            }
            _ => None,
        };

        let mid = self
            .free_ranges
            .iter()
            .zip(self.free_ranges.iter().skip(1))
            .map(|(ra, rb)| ra.end..rb.start);

        first.into_iter().chain(mid).chain(last)
    }

    /// Frees all allocations, restoring the allocator to its initial state.
    ///
    /// # Example
    ///
    /// ```
    /// use range_alloc::RangeAllocator;
    ///
    /// let mut alloc = RangeAllocator::new(0..10);
    /// alloc.allocate_range(10).unwrap();
    /// alloc.reset();
    /// assert!(alloc.is_empty());
    /// ```
    pub fn reset(&mut self) {
        self.free_ranges.clear();
        self.free_ranges.push(self.initial_range.clone());
    }

    /// Returns `true` if nothing is currently allocated.
    pub fn is_empty(&self) -> bool {
        self.free_ranges.len() == 1 && self.free_ranges[0] == self.initial_range
    }
}

impl<T: Copy + Sub<Output = T> + Sum> RangeAllocator<T> {
    /// Returns the total length of all free ranges combined.
    ///
    /// This may be spread across multiple non-contiguous ranges, so an
    /// allocation of this size is not guaranteed to succeed.
    ///
    /// # Example
    ///
    /// ```
    /// use range_alloc::RangeAllocator;
    ///
    /// let mut alloc = RangeAllocator::new(0..100);
    /// alloc.allocate_range(30).unwrap();
    /// assert_eq!(alloc.total_available(), 70);
    /// ```
    pub fn total_available(&self) -> T {
        self.free_ranges
            .iter()
            .map(|range| range.end - range.start)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_allocation() {
        let mut alloc = RangeAllocator::new(0..10);
        // Test if an allocation works
        assert_eq!(alloc.allocate_range(4), Ok(0..4));
        assert!(alloc.allocated_ranges().eq(std::iter::once(0..4)));
        // Free the prior allocation
        alloc.free_range(0..4);
        // Make sure the free actually worked
        assert_eq!(alloc.free_ranges, vec![0..10]);
        assert!(alloc.allocated_ranges().eq(std::iter::empty()));
    }

    #[test]
    fn test_out_of_space() {
        let mut alloc = RangeAllocator::new(0..10);
        // Test if the allocator runs out of space correctly
        assert_eq!(alloc.allocate_range(10), Ok(0..10));
        assert!(alloc.allocated_ranges().eq(std::iter::once(0..10)));
        assert!(alloc.allocate_range(4).is_err());
        alloc.free_range(0..10);
    }

    #[test]
    fn test_grow() {
        let mut alloc = RangeAllocator::new(0..11);
        // Test if the allocator runs out of space correctly
        assert_eq!(alloc.allocate_range(10), Ok(0..10));
        assert!(alloc.allocated_ranges().eq(std::iter::once(0..10)));
        assert!(alloc.allocate_range(4).is_err());
        alloc.grow_to(20);
        assert_eq!(alloc.allocate_range(4), Ok(10..14));
        alloc.free_range(0..14);
    }

    #[test]
    fn test_grow_with_hole_at_start() {
        let mut alloc = RangeAllocator::new(0..6);

        assert_eq!(alloc.allocate_range(3), Ok(0..3));
        assert_eq!(alloc.allocate_range(3), Ok(3..6));
        alloc.free_range(0..3);

        alloc.grow_to(9);
        assert_eq!(alloc.allocated_ranges().collect::<Vec<_>>(), [3..6]);
    }
    #[test]
    fn test_grow_with_hole_in_middle() {
        let mut alloc = RangeAllocator::new(0..6);

        assert_eq!(alloc.allocate_range(2), Ok(0..2));
        assert_eq!(alloc.allocate_range(2), Ok(2..4));
        assert_eq!(alloc.allocate_range(2), Ok(4..6));
        alloc.free_range(2..4);

        alloc.grow_to(9);
        assert_eq!(alloc.allocated_ranges().collect::<Vec<_>>(), [0..2, 4..6]);
    }

    #[test]
    fn test_dont_use_block_that_is_too_small() {
        let mut alloc = RangeAllocator::new(0..10);
        // Allocate three blocks then free the middle one and check for correct state
        assert_eq!(alloc.allocate_range(3), Ok(0..3));
        assert_eq!(alloc.allocate_range(3), Ok(3..6));
        assert_eq!(alloc.allocate_range(3), Ok(6..9));
        alloc.free_range(3..6);
        assert_eq!(alloc.free_ranges, vec![3..6, 9..10]);
        assert_eq!(
            alloc.allocated_ranges().collect::<Vec<Range<i32>>>(),
            vec![0..3, 6..9]
        );
        // Now request space that the middle block can fill, but the end one can't.
        assert_eq!(alloc.allocate_range(3), Ok(3..6));
    }

    #[test]
    fn test_free_blocks_in_middle() {
        let mut alloc = RangeAllocator::new(0..100);
        // Allocate many blocks then free every other block.
        assert_eq!(alloc.allocate_range(10), Ok(0..10));
        assert_eq!(alloc.allocate_range(10), Ok(10..20));
        assert_eq!(alloc.allocate_range(10), Ok(20..30));
        assert_eq!(alloc.allocate_range(10), Ok(30..40));
        assert_eq!(alloc.allocate_range(10), Ok(40..50));
        assert_eq!(alloc.allocate_range(10), Ok(50..60));
        assert_eq!(alloc.allocate_range(10), Ok(60..70));
        assert_eq!(alloc.allocate_range(10), Ok(70..80));
        assert_eq!(alloc.allocate_range(10), Ok(80..90));
        assert_eq!(alloc.allocate_range(10), Ok(90..100));
        assert_eq!(alloc.free_ranges, vec![]);
        assert!(alloc.allocated_ranges().eq(std::iter::once(0..100)));
        alloc.free_range(10..20);
        alloc.free_range(30..40);
        alloc.free_range(50..60);
        alloc.free_range(70..80);
        alloc.free_range(90..100);
        // Check that the right blocks were freed.
        assert_eq!(
            alloc.free_ranges,
            vec![10..20, 30..40, 50..60, 70..80, 90..100]
        );
        assert_eq!(
            alloc.allocated_ranges().collect::<Vec<Range<i32>>>(),
            vec![0..10, 20..30, 40..50, 60..70, 80..90]
        );
        // Fragment the memory on purpose a bit.
        assert_eq!(alloc.allocate_range(6), Ok(10..16));
        assert_eq!(alloc.allocate_range(6), Ok(30..36));
        assert_eq!(alloc.allocate_range(6), Ok(50..56));
        assert_eq!(alloc.allocate_range(6), Ok(70..76));
        assert_eq!(alloc.allocate_range(6), Ok(90..96));
        // Check for fragmentation.
        assert_eq!(
            alloc.free_ranges,
            vec![16..20, 36..40, 56..60, 76..80, 96..100]
        );
        assert_eq!(
            alloc.allocated_ranges().collect::<Vec<Range<i32>>>(),
            vec![0..16, 20..36, 40..56, 60..76, 80..96]
        );
        // Fill up the fragmentation
        assert_eq!(alloc.allocate_range(4), Ok(16..20));
        assert_eq!(alloc.allocate_range(4), Ok(36..40));
        assert_eq!(alloc.allocate_range(4), Ok(56..60));
        assert_eq!(alloc.allocate_range(4), Ok(76..80));
        assert_eq!(alloc.allocate_range(4), Ok(96..100));
        // Check that nothing is free.
        assert_eq!(alloc.free_ranges, vec![]);
        assert!(alloc.allocated_ranges().eq(std::iter::once(0..100)));
    }

    #[test]
    fn test_ignore_block_if_another_fits_better() {
        let mut alloc = RangeAllocator::new(0..10);
        // Allocate blocks such that the only free spaces available are 3..6 and 9..10
        // in order to prepare for the next test.
        assert_eq!(alloc.allocate_range(3), Ok(0..3));
        assert_eq!(alloc.allocate_range(3), Ok(3..6));
        assert_eq!(alloc.allocate_range(3), Ok(6..9));
        alloc.free_range(3..6);
        assert_eq!(alloc.free_ranges, vec![3..6, 9..10]);
        assert_eq!(
            alloc.allocated_ranges().collect::<Vec<Range<i32>>>(),
            vec![0..3, 6..9]
        );
        // Now request space that can be filled by 3..6 but should be filled by 9..10
        // because 9..10 is a perfect fit.
        assert_eq!(alloc.allocate_range(1), Ok(9..10));
    }

    #[test]
    fn test_merge_neighbors() {
        let mut alloc = RangeAllocator::new(0..9);
        assert_eq!(alloc.allocate_range(3), Ok(0..3));
        assert_eq!(alloc.allocate_range(3), Ok(3..6));
        assert_eq!(alloc.allocate_range(3), Ok(6..9));
        alloc.free_range(0..3);
        alloc.free_range(6..9);
        alloc.free_range(3..6);
        assert_eq!(alloc.free_ranges, vec![0..9]);
        assert!(alloc.allocated_ranges().eq(std::iter::empty()));
    }

    #[test]
    fn test_aligned_already_aligned() {
        let mut alloc = RangeAllocator::new(0..20);
        // Start is already aligned to 4, no padding needed.
        assert_eq!(alloc.allocate_range_aligned(4, 4), Ok(0..4));
        assert_eq!(alloc.free_ranges, vec![4..20]);
    }

    #[test]
    fn test_aligned_with_padding() {
        let mut alloc = RangeAllocator::new(0..20);
        // Occupy 1 byte to offset the free range start.
        assert_eq!(alloc.allocate_range(1), Ok(0..1));
        // Free range is now 1..20. Alignment 4 rounds up to 4.
        assert_eq!(alloc.allocate_range_aligned(4, 4), Ok(4..8));
        // Prefix 1..4 and suffix 8..20 remain free.
        assert_eq!(alloc.free_ranges, vec![1..4, 8..20]);
    }

    #[test]
    fn test_aligned_prefix_is_reusable() {
        let mut alloc = RangeAllocator::new(0..20);
        assert_eq!(alloc.allocate_range(1), Ok(0..1));
        assert_eq!(alloc.allocate_range_aligned(4, 4), Ok(4..8));
        // The prefix 1..4 should be usable for a smaller allocation.
        assert_eq!(alloc.allocate_range(3), Ok(1..4));
        assert_eq!(alloc.free_ranges, vec![8..20]);
    }

    #[test]
    fn test_aligned_no_fit() {
        let mut alloc = RangeAllocator::new(0..5);
        assert_eq!(alloc.allocate_range(1), Ok(0..1));
        // Free range is 1..5. Alignment 4 rounds to 4, usable = 5-4 = 1 < 4.
        assert!(alloc.allocate_range_aligned(4, 4).is_err());
    }

    #[test]
    fn test_aligned_exact_fit_after_padding() {
        let mut alloc = RangeAllocator::new(0..8);
        assert_eq!(alloc.allocate_range(1), Ok(0..1));
        // Free range is 1..8. Alignment 4 rounds to 4, usable = 8-4 = 4 == 4.
        assert_eq!(alloc.allocate_range_aligned(4, 4), Ok(4..8));
        // Prefix 1..4 remains, suffix consumed entirely.
        assert_eq!(alloc.free_ranges, vec![1..4]);
    }

    #[test]
    fn test_aligned_best_fit() {
        let mut alloc = RangeAllocator::new(0..32);
        // Create two gaps with different usable sizes after alignment.
        assert_eq!(alloc.allocate_range(4), Ok(0..4));
        assert_eq!(alloc.allocate_range(4), Ok(4..8));
        assert_eq!(alloc.allocate_range(4), Ok(8..12));
        assert_eq!(alloc.allocate_range(4), Ok(12..16));
        assert_eq!(alloc.allocate_range(4), Ok(16..20));
        assert_eq!(alloc.allocate_range(12), Ok(20..32));

        // Free two ranges: 4..8 (already aligned to 4, usable 4) and
        // 12..20 (already aligned to 4, usable 8).
        alloc.free_range(4..8);
        alloc.free_range(12..20);
        assert_eq!(alloc.free_ranges, vec![4..8, 12..20]);

        // Allocate 4 aligned to 4. Both fit, but 4..8 is the tighter fit.
        assert_eq!(alloc.allocate_range_aligned(4, 4), Ok(4..8));
    }

    #[test]
    fn test_aligned_non_power_of_two() {
        let mut alloc = RangeAllocator::new(0..20);
        assert_eq!(alloc.allocate_range(1), Ok(0..1));
        // Free range: 1..20. Alignment 3 rounds 1 up to 3.
        assert_eq!(alloc.allocate_range_aligned(2, 3), Ok(3..5));
        assert_eq!(alloc.free_ranges, vec![1..3, 5..20]);
    }

    #[test]
    fn test_aligned_multiple_allocations() {
        let mut alloc = RangeAllocator::new(0..32);
        assert_eq!(alloc.allocate_range_aligned(4, 8), Ok(0..4));
        // Free: 4..32. Next align-8 start is 8.
        assert_eq!(alloc.allocate_range_aligned(4, 8), Ok(8..12));
        // Free: 4..8, 12..32. Next align-8 start in 12..32 is 16.
        assert_eq!(alloc.allocate_range_aligned(4, 8), Ok(16..20));
        // Free: 4..8, 12..16, 20..32.
        assert_eq!(alloc.free_ranges, vec![4..8, 12..16, 20..32]);
    }

    #[test]
    fn test_aligned_allocation_then_free_merges() {
        let mut alloc = RangeAllocator::new(0..16);
        assert_eq!(alloc.allocate_range(1), Ok(0..1));
        assert_eq!(alloc.allocate_range_aligned(4, 4), Ok(4..8));
        // Free: 1..4, 8..16
        // Free the aligned range; it should not merge (not adjacent to either).
        alloc.free_range(4..8);
        // 1..4 and 4..8 merge into 1..8, then 1..8 and 8..16 merge into 1..16.
        assert_eq!(alloc.free_ranges, vec![1..16]);
    }

    #[test]
    fn test_allocate_range_delegates_correctly() {
        // Verify allocate_range still behaves identically to the original.
        let mut alloc = RangeAllocator::new(0..10);
        assert_eq!(alloc.allocate_range(4), Ok(0..4));
        assert_eq!(alloc.allocate_range(3), Ok(4..7));
        assert_eq!(alloc.free_ranges, vec![7..10]);
        alloc.free_range(0..4);
        assert_eq!(alloc.free_ranges, vec![0..4, 7..10]);
    }
}
