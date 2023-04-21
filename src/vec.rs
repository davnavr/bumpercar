//! Contains a version of [`Vec<T>`](self::Vec) that allocates into an
//! [`Arena`](crate::Arena).

use crate::Bump;
use core::ptr::NonNull;

/// A contiguous, growable array type that is allocated into an [`Arena`](crate::Arena).
///
/// A best effort is made to provide the same methods as [`Vec<T>`](alloc::vec::Vec<T>) in the standard library.
pub struct Vec<'alloc, 'arena, T, A = crate::Allocator<'arena>>
where
    A: Bump<'alloc, 'arena>
{
    pointer: NonNull<T>,
    capacity: usize,
    length: usize,
    allocator: &'alloc A, // A, have &impl Bump implement Bump?
}

impl<'alloc, 'arena, T, A> Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>
{
    #[inline]
    pub fn allocator(&self) -> &'alloc A {
        self.allocator
    }

    #[inline]
    pub fn new_in(&self, allocator: &'alloc A) -> Self {
        Self {
            pointer: NonNull::dangling(),
            capacity: 0,
            length: 0,
            allocator,
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.pointer.as_ptr()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.pointer.as_ptr()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    //pub fn leak(self) -> &'arena mut [T] {}

    /// Appends an item to the end of the collection.
    pub fn push(&mut self, value: T) {
        if self.length == self.capacity {
            todo!("realloc");
        }

        // Safety: pointer is valid, since capacity check occurs above
        // Safety: overflow assumed not to occur since realloc would panic for sufficently large allocations
        let ptr = unsafe {
            let ptr = self.as_mut_ptr().add(self.length);
        };

        // Safety: pointer is valid, aligned, and does not contain an initialized value
        unsafe {
            core::ptr::write(ptr, value);
            self.length += 1;
        }
    }
}

impl<'alloc, 'arena, T, A> core::ops::Deref for Vec<'alloc, 'arena, T, A> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        // Safety: validity of pointer and length is an invariant maintained by other code
        unsafe {
            core::slice::from_raw_parts(self.as_ptr(), self.length)
        }
    }
}

impl<'alloc, 'arena, T, A> core::ops::DerefMut for Vec<'alloc, 'arena, T, A> {
    #[inline]
    fn deref_mut(&self) -> &mut [T] {
        // Safety: validity of pointer and length is an invariant maintained by other code
        unsafe {
            core::slice::from_raw_parts_mut(self.as_ptr(), self.length)
        }
    }
}
