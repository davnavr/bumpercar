//! Contains the [`Bump`] trait.

use core::alloc::Layout;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

mod private {
    pub trait Sealed {}

    impl Sealed for crate::Allocator<'_> {}
    impl Sealed for crate::Frame<'_, '_> {}

    #[cfg(feature = "sync")]
    impl Sealed for crate::sync::ThreadAllocator<'_> {}
}

/// Contains methods for bump allocation.
///
/// # Safety
///
/// Implementations must ensure that all allocations are valid for the lifetime `'a`.
///
/// Any pointers (such as those originating from [`alloc_with_layout`](Bump::alloc_with_layout))
/// returned as a result of allocation requests must be
/// [valid](https://doc.rust-lang.org/core/ptr/#safety).
///
/// Additionally, requests to allocate zero-sized values must yield a pointer that can be
/// transmuted into a valid mutable reference.
pub unsafe trait Bump<'me, 'a>: private::Sealed {
    /// Calls a closure with a [`Frame`](crate::Frame) used to tie the lifetime of allocations made
    /// into an arena to a stack frame.
    fn with_frame<T, F: FnOnce(&mut crate::Frame) -> T>(&'me mut self, f: F) -> T;

    /// Allocates space for an object with the given [`Layout`], returning a valid pointer to it.
    ///
    /// # Panics
    ///
    /// Panics if any calls to an underlying memory allocator fail.
    fn alloc_with_layout(&'me self, layout: Layout) -> NonNull<u8>;

    /// Allocates space for an object with the given [`Layout`], and passes the pointer to a
    /// closure that returns a [`Result<T>`] or [`Option<T>`].
    ///
    /// If the closure returns [`Err`] or [`None`], then the object is deallocated.
    ///
    /// # Safety
    ///
    /// Callers must ensure that any error value returned by the closure does not contain
    /// pointers into the allocated object, since it would be immediately deallocated.
    unsafe fn alloc_try_with_layout<R, F>(&'me self, layout: Layout, f: F) -> R
    where
        R: crate::private::Try,
        F: FnOnce(NonNull<u8>) -> R;

    /// Allocates space for an instance of `T`.
    #[inline(always)]
    fn alloc_uninit<T>(&'me self) -> &'a mut MaybeUninit<T> {
        // Safety: passed layout ensures proper alignment
        unsafe { self.alloc_with_layout(Layout::new::<T>()).cast().as_mut() }
    }

    /// Allocates space for an instance of `T`, and provides a pointer to `T` to a closure.
    ///
    /// If the closure returns [`Err`] or [`None`], then the object is deallocated.
    ///
    /// See [`alloc_try_with_layout`](Bump::alloc_try_with_layout) for more information.
    #[inline(always)]
    fn alloc_try_uninit<T, R, F>(&'me self, f: F) -> R
    where
        T: 'a,
        R: crate::private::Try,
        F: FnOnce(&'a mut MaybeUninit<T>) -> R,
    {
        let alloc_f = |pointer: NonNull<u8>| {
            f({
                // Safety: passed layout ensures pointer can be turned into a valid reference
                unsafe { pointer.cast().as_mut() }
            })
        };

        // Safety: parameter of F is tied to a lifetime, errors won't cause dangling pointers
        unsafe { self.alloc_try_with_layout::<R, _>(Layout::new::<T>(), alloc_f) }
    }

    /// Allocates space for an instance of `T`, and initializes it with the given closure.
    #[inline(always)]
    fn alloc_with<T, F: FnOnce() -> T>(&'me self, f: F) -> &'a mut T {
        self.alloc_uninit::<T>().write(f())
    }

    /// Allocates space for an instance of `T`, and moves the value into the allocation.
    #[inline(always)]
    fn alloc<T>(&'me self, value: T) -> &'a mut T {
        self.alloc_with(|| value)
    }

    /// Allocates space for a slice of `T` with the given `length`.
    #[inline(always)]
    fn alloc_slice_uninit<T>(&'me self, length: usize) -> &'a mut [MaybeUninit<T>] {
        let allocation = self.alloc_with_layout(Layout::array::<T>(length).unwrap());

        // Safety: layout ensures length is valid, allocation is a valid pointer
        unsafe {
            core::slice::from_raw_parts_mut::<'a, _>(
                allocation.cast::<MaybeUninit<T>>().as_ptr(),
                length,
            )
        }
    }

    /// Allocates space to store the given slice, and copies the slice into the arena.
    #[inline(always)]
    fn alloc_slice<T: Copy>(&'me self, slice: &[T]) -> &'a mut [T] {
        let destination = self.alloc_slice_uninit::<T>(slice.len());

        // Safety: [T] and [MaybeUninit<T>] have the same layout
        unsafe {
            let source: &[MaybeUninit<T>] = core::mem::transmute::<&[T], _>(slice);

            destination.copy_from_slice(source);

            core::mem::transmute::<_, &'a mut [T]>(destination)
        }
    }

    /// Allocates space to store a string, and copies it into the arena.
    #[inline(always)]
    fn alloc_str(&'me self, s: &str) -> &'a mut str {
        let bytes = self.alloc_slice(s.as_bytes());
        unsafe {
            // Safety: Bytes are already valid UTF-8
            core::str::from_utf8_unchecked_mut(bytes)
        }
    }

    /// Allocates space to store the given slice, cloning each item into the arena.
    fn alloc_slice_cloned<T: Clone>(&'me self, slice: &[T]) -> &'a mut [T] {
        let destination = self.alloc_slice_uninit::<T>(slice.len());

        for i in 0..slice.len() {
            destination[i].write(slice[i].clone());
        }

        // Safety: [T] and [MaybeUninit<T>] have the same layout, destination is initialized
        unsafe { core::mem::transmute::<&'a mut [MaybeUninit<T>], &'a mut [T]>(destination) }
    }

    /// Allocates a slice to contain the items yielded by the iterator.
    ///
    /// # Panics
    ///
    /// Panics if enough memory to contain the slice could not be allocated, or if the iterator
    /// yielded more items than it said it would.
    fn alloc_slice_from_iter<T, I>(&'me self, items: I) -> &'a mut [T]
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        let items_iter = items.into_iter();
        let destination = self.alloc_slice_uninit::<I::Item>(items_iter.len());

        let mut actual_length = 0usize;
        let mut destination_iter = destination.iter_mut();
        for value in items_iter {
            destination_iter
                .next()
                .expect("iterator yielded too many items")
                .write(value);
            actual_length += 1;
        }

        // If iterator "lies" and returns too few items, then slice
        let slice = &mut destination[0..actual_length];

        // Safety: [T] and [MaybeUninit<T>] have the same layout, slice is initialized
        unsafe { core::mem::transmute::<&'a mut [MaybeUninit<T>], &'a mut [T]>(slice) }
    }
}
