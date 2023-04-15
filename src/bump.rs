//! Contains the [`Bump`] trait.

use core::alloc::Layout;
use core::ptr::NonNull;

mod sealed {
    pub trait Sealed {}
}

/// Contains methods for bump allocation.
///
/// # Safety
///
/// Implementations must ensure that all allocations are valid for the lifetime `'a`.
///
/// Any pointers (such as those originating from [`alloc_with_layout`](Bump::alloc_with_layout))
/// returned as a result of allocation requests must be [valid](https://doc.rust-lang.org/std/ptr/#safety).
///
/// Additionally, requests to allocate zero-sized values must yield a pointer that can be
/// transmuted into a valid mutable reference.
pub unsafe trait Bump<'me, 'a>: sealed::Sealed {
    /// Allocates space for an object with the given [`Layout`], returning a valid pointer to it.
    ///
    /// # Panics
    ///
    /// Panics if any calls to an underlying memory allocator fail.
    fn alloc_with_layout(&'me self, layout: Layout) -> NonNull<u8>;

    /// Allocates space for an instance of `T`, and initializes it with the given closure.
    #[inline(always)]
    fn alloc_with<T, F: FnOnce() -> T>(&'me self, f: F) -> &'a mut T {
        let mut pointer = self.alloc_with_layout(Layout::new::<T>()).cast::<T>();
        unsafe {
            // Safety: passed layout ensures proper alignment
            std::ptr::write(pointer.as_ptr(), f());

            // Safety: previous line ensures T is initialized
            pointer.as_mut()
        }
    }

    /// Allocates space for an instance of `T`, and moves the value into the allocation.
    #[inline(always)]
    fn alloc<T>(&'me self, value: T) -> &'a mut T {
        self.alloc_with(|| value)
    }
}
