use crate::Bump;
use core::ptr::NonNull;

/// An iterator that moves all items out of a [`Vec<T>`](crate::vec::Vec).
pub struct IntoIter<'alloc, T, A> {
    start: NonNull<T>,
    /// Points past the last element in the vector.
    end: NonNull<T>,
    capacity: usize,
    current: NonNull<T>,
    allocator: &'alloc A,
}

impl<'arena, 'alloc, T, A> IntoIter<'alloc, T, A>
where
    A: Bump<'alloc, 'arena>,
{
    /// # Safety
    ///
    /// - `length` must be less than or equal to `capacity`.
    /// - `pointer + length` must not overflow
    /// - `pointer` must be allocated by the `allocator`.
    pub(in crate::vec) unsafe fn new(
        pointer: NonNull<T>,
        length: usize,
        capacity: usize,
        allocator: &'alloc A,
    ) -> Self {
        debug_assert!(length <= capacity);

        // Safety: caller should prevent overflow
        let end = unsafe { NonNull::new_unchecked(pointer.as_ptr().add(length)) };

        debug_assert!(end >= pointer);

        Self {
            start: pointer,
            current: pointer,
            allocator,
            capacity,
            end,
        }
    }

    fn len(&self) -> usize {
        ((self.end.as_ptr() as usize) - (self.current.as_ptr() as usize))
            / core::mem::size_of::<T>()
    }

    /// Returns the [`Bump`] allocator used to allocate the items in an arena.
    pub fn allocator(&self) -> &'alloc A {
        self.allocator
    }

    /// Returns the remaining items in the vector as a slice.
    pub fn as_slice(&self) -> &[T] {
        // Safety: Pointer is valid and points to valid items for length
        unsafe { core::slice::from_raw_parts(self.current.as_ptr(), self.len()) }
    }

    /// Returns the remaining items in the vector as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // Safety: Pointer is valid and points to valid items for length
        unsafe { core::slice::from_raw_parts_mut(self.current.as_ptr(), self.len()) }
    }
}

impl<'arena, 'alloc, T, A> core::iter::Iterator for IntoIter<'alloc, T, A>
where
    A: Bump<'alloc, 'arena>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            return None;
        }

        // Safety: pointer is aligned and contains a valid value
        let item = unsafe { core::ptr::read(self.current.as_ptr()) };

        // Safety: overflow assumed not to occur
        self.current = unsafe { NonNull::new_unchecked(self.current.as_ptr().add(1)) };

        Some(item)
    }
}

// TODO: drop impl
//impl<'arena, 'alloc, T, A> core::ops::Drop

impl<'arena, 'alloc, T, A> core::fmt::Debug for IntoIter<'alloc, T, A>
where
    A: Bump<'alloc, 'arena>,
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}
