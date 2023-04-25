use crate::Bump;
use core::ptr::NonNull;

/// An iterator that moves all items out of a [`Vec<T>`](crate::vec::Vec).
pub struct IntoIter<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
{
    start: NonNull<T>,
    /// Points past the last element in the vector.
    end: NonNull<T>,
    capacity: usize,
    current: NonNull<T>,
    allocator: &'alloc A,
    _marker: core::marker::PhantomData<T>,
}

impl<'alloc, T, A> IntoIter<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
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
            _marker: core::marker::PhantomData,
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

    #[inline(always)]
    unsafe fn drop_impl(&mut self) -> Result<(), ()> {
        let elements: *mut [T] = self.as_mut_slice();

        // Safety: Caller ensures elements are initialized.
        unsafe {
            core::ptr::drop_in_place(elements);
        }

        // Safety: elements have already been dropped
        unsafe {
            self.allocator.realloc(
                self.start.cast(),
                core::alloc::Layout::array::<T>(self.capacity).map_err(|_| ())?,
                0,
            );
        }

        Ok(())
    }
}

impl<'alloc, T, A> core::iter::Iterator for IntoIter<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
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

impl<'alloc, T, A> core::ops::Drop for IntoIter<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
{
    #[inline]
    fn drop(&mut self) {
        // If an error occurs while dropping, then the memory is just never freed

        // Safety: Remaining vector elements are being dropped, they are still valid at this point
        let _ = unsafe { self.drop_impl() };
    }
}

impl<'alloc, T, A> core::fmt::Debug for IntoIter<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}
