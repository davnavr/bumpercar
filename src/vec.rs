//! Contains a version of [`Vec<T>`](self::Vec) that allocates into an
//! [`Arena`](crate::Arena).

use crate::raw_arena::OutOfMemory;
use crate::Bump;
use core::alloc::Layout;
use core::ptr::NonNull;

mod partial_eq;

/// A contiguous, growable array type that is allocated into an [`Arena`](crate::Arena).
///
/// A best effort is made to provide the same methods as [`Vec<T>`](alloc::vec::Vec<T>) in the
/// standard library.
///
/// # Examples
///
/// ```
/// use bumpercar::prelude::*;
/// use bumpercar::vec::Vec;
///
/// let mut arena = Arena::new();
/// let allocator = arena.allocator();
///
/// let mut vec = Vec::new_in(&allocator);
/// vec.push(b'a');
/// vec.push(b'b');
/// vec.push(b'c');
///
/// assert_eq!(vec.len(), 3);
/// assert_eq!(b"abc", vec.as_slice());
/// ```
pub struct Vec<'alloc, 'arena, T, A = crate::Allocator<'arena>>
where
    A: Bump<'alloc, 'arena>,
{
    pointer: NonNull<T>,
    /// The maximum number of items that this vector can contain. Must always be greater than or
    /// equal to [`length`](Vec::length).
    capacity: usize,
    length: usize,
    allocator: &'alloc A,
    _phantom: core::marker::PhantomData<&'arena T>,
}

impl<'alloc, 'arena, T, A> Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
{
    const ELEMENT_SIZE: usize = core::mem::size_of::<T>();

    /// Gets the allocator used to contain the vector's elements.
    #[inline]
    pub fn allocator(&self) -> &'alloc A {
        self.allocator
    }

    /// Creates an empty vector that will allocate memory with the given [`Bump`] allocator.
    #[inline]
    pub fn new_in(allocator: &'alloc A) -> Self {
        Self {
            pointer: NonNull::dangling(),
            capacity: 0,
            length: 0,
            allocator,
            _phantom: core::marker::PhantomData,
        }
    }

    /// Gets a raw pointer to the vector's contents.
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.pointer.as_ptr()
    }

    /// Gets a mutable pointer to the vector's contents.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.pointer.as_ptr()
    }

    /// Gets the maximum number of items the vector can contain before a growth is required.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Gets the number of items in the vector.
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns `true` if the vector is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Gets a slice containing the vector's elements.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self
    }

    /// Gets a mutable slice containing the vector's elements.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }

    /// Consumes and leaks the [`Vec<T>`].
    ///
    /// Note that this will prevent any destructors for `T` from being run.
    #[inline]
    pub fn leak(mut self) -> &'arena mut [T] {
        // Safety: slice was allocated in the arena, so returned reference will live as long as the arena
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.length) }
    }

    /// Removes all values from the vector.
    #[inline]
    pub fn clear(&mut self) {
        let elements = NonNull::from(self.as_mut_slice());

        self.length = 0;

        // Safety: elements contains valid values
        unsafe {
            core::ptr::drop_in_place(elements.as_ptr());
        }
    }

    #[inline]
    fn try_reserve_exact(&mut self, additional: usize) -> Result<(), OutOfMemory> {
        if self.capacity - self.length >= additional || Self::ELEMENT_SIZE == 0 {
            return Ok(());
        }

        // This won't overflow, capacity uses checked_add later
        let current_size = Self::ELEMENT_SIZE
            .checked_mul(self.capacity)
            .ok_or(OutOfMemory)?;

        // Safety: no dangling pointers
        let (new_pointer, _) = unsafe {
            self.allocator.realloc(
                self.pointer.cast(),
                Layout::from_size_align(current_size, core::mem::align_of::<T>())?,
                current_size
                    .checked_add(
                        Self::ELEMENT_SIZE
                            .checked_mul(additional)
                            .ok_or(OutOfMemory)?,
                    )
                    .ok_or(OutOfMemory)?,
            )
        };

        self.pointer = new_pointer.cast();
        Ok(())
    }

    /// Reserves space for an exact number of additional elements, allocating new memory as needed.
    pub fn reserve_exact(&mut self, additional: usize) {
        self.try_reserve_exact(additional).unwrap()
    }

    /// Appends an item to the end of the vector.
    #[inline]
    pub fn push(&mut self, value: T) {
        if self.length == self.capacity {
            self.reserve_exact(core::cmp::max(self.length, 1)); // TODO: Growth should be 0, 4, 8, ...
        }

        // Safety: pointer is valid, since capacity check occurs above
        // Safety: overflow assumed not to occur since realloc would panic for sufficently large allocations
        let ptr = unsafe { self.as_mut_ptr().add(self.length) };

        // Safety: pointer is valid, aligned, and does not contain an initialized value
        unsafe {
            core::ptr::write(ptr, value);
            self.length += 1;
        }
    }

    /// Removes the last item of the vector and returns it, or [`None`] if the vector is empty.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.length == 0 {
            return None;
        }

        self.length -= 1;

        // Safety: overflow shouldn't occur, length - 1 <= capacity
        let ptr = unsafe { self.as_mut_ptr().add(self.length) };

        // Safety: pointer is valid, aligned, and contains an initialized value
        unsafe { Some(core::ptr::read(ptr)) }
    }
}

impl<'alloc, 'arena, T, A> core::ops::Deref for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
{
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        // Safety: validity of pointer and length is an invariant maintained by other code
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.length) }
    }
}

impl<'alloc, 'arena, T, A> core::ops::DerefMut for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        // Safety: validity of pointer and length is an invariant maintained by other code
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.length) }
    }
}

impl<'alloc, 'arena, T, A> core::ops::Drop for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
{
    fn drop(&mut self) {
        self.clear();
    }
}

impl<'alloc, 'arena, T, A> core::cmp::Eq for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
    T: core::cmp::Eq,
{
}

impl<'alloc, 'arena, T, A> core::fmt::Debug for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_slice(), f)
    }
}
