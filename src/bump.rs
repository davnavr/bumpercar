//! Contains the [`Bump`] trait.

use crate::private::Try;
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
/// The sole exception to this are objects passed to [`realloc`]. See the documentation for
/// [`realloc`] for more information.
///
/// Any pointers (such as those originating from [`alloc_with_layout`](Bump::alloc_with_layout))
/// returned as a result of allocation requests must be
/// [valid](https://doc.rust-lang.org/core/ptr/#safety).
///
/// Additionally, requests to allocate zero-sized values must yield a pointer that can be
/// transmuted into a valid mutable reference.
///
/// [`realloc`]: Bump::realloc
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

    /// Attempts to shrink or grow an allocated object. Returns a pointer to the re-allocated
    /// object, and an additional pointer to freed memory that can be reused. After the call,
    /// **the original pointer will most likely be invalid**, refer to the
    /// [safety documentation](Bump::realloc#Safety) for more information.
    ///
    /// If reallocation cannot occur in the current chunk, a new allocation is made in a new chunk, and
    /// the now freed memory is returned as the second pointer.
    ///
    /// If a shrink occurs, a second pointer is always returned.
    ///
    /// If the new size is same as the current size of the object, then no allocation occurs.
    ///
    /// # Safety
    ///
    /// This function is **very** unsafe, as it can easily cause use-after-free bugs. Callers must
    /// ensure that the pointer was obtained by allocating into the **same arena**.
    ///
    /// If a second pointer to reusable memory is returned, callers **must** re-initialize the
    /// memory if they wish to reuse it, as the pointed-to memory would be logically uninitialized
    /// after the call to [`realloc`].
    ///
    /// After the call to [`realloc`], the `pointer` parameter will **only** be valid if no
    /// reallocation occured. The returned pointer now refers to the re-allocated object.
    ///
    /// Despite the fact that reallocation may result in an object being move, callers must still
    /// ensure that all objects do not outlive the arena lifetime `'a`.
    ///
    /// # Panics
    ///
    /// Panics if allocation was required, and failed.
    /// See [`alloc_with_layout`](Bump::alloc_with_layout) for more information.
    ///
    /// [`realloc`]: Bump::realloc
    unsafe fn realloc(
        &'me self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_size: usize,
    ) -> (NonNull<u8>, Option<NonNull<u8>>);

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
        R: Try,
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
        R: Try,
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
        // Safety: Bytes are already valid UTF-8
        unsafe { core::str::from_utf8_unchecked_mut(bytes) }
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

    /// Allocates space to store a slice, cloning the given `value` to fill it.
    ///
    /// # Example
    ///
    /// ```
    /// use bumpercar::prelude::*;
    ///
    /// let mut arena = Arena::new();
    /// let allocator = arena.allocator();
    /// let items = allocator.alloc_slice_fill(3usize, 42u8);
    /// assert_eq!(items, &[42u8, 42, 42]);
    /// ```
    fn alloc_slice_fill<T: Clone>(&'me self, length: usize, value: T) -> &'a mut [T] {
        let destination = self.alloc_slice_uninit::<T>(length);
        if let Some((last, head)) = destination.split_last_mut() {
            for item in head.iter_mut() {
                item.write(value.clone());
            }

            last.write(value);

            // Safety: [T] and [MaybeUninit<T>] have the same layout, destination is initialized
            unsafe { core::mem::transmute::<&'a mut [MaybeUninit<T>], &'a mut [T]>(destination) }
        } else {
            Default::default()
        }
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

    /// Allocates a slice of the specified `length`, passing indices to a closure to obtain values to
    /// fill the slice.
    fn alloc_slice_with<T, F: FnMut(usize) -> T>(
        &'me self,
        length: usize,
        mut f: F,
    ) -> &'a mut [T] {
        let destination = self.alloc_slice_uninit::<T>(length);

        for (i, item) in destination.iter_mut().enumerate() {
            item.write(f(i));
        }

        // Safety: [T] and [MaybeUninit<T>] have the same layout, destination is initialized
        unsafe { core::mem::transmute::<&'a mut [MaybeUninit<T>], &'a mut [T]>(destination) }
    }

    /// Allocates a slice of the specified `length`, using a closure to attempt to obtain values to fill the slice.
    ///
    /// The closure receives the index of the item, and may return [`None`] or [`Err`] to deallocate the slice.
    fn alloc_slice_try_with<T, E, F>(&'me self, length: usize, mut f: F) -> Result<&'a mut [T], E>
    where
        F: FnMut(usize) -> Result<T, E>,
    {
        let attempt = move |allocation: NonNull<u8>| -> Result<&'a mut [T], E> {
            // Safety: layout ensures length is valid, allocation is a valid pointer
            let destination = unsafe {
                core::slice::from_raw_parts_mut::<'a, _>(
                    allocation.cast::<MaybeUninit<T>>().as_ptr(),
                    length,
                )
            };

            for (i, item) in destination.iter_mut().enumerate() {
                item.write(f(i)?);
            }

            // Safety: [T] and [MaybeUninit<T>] have the same layout, destination is fully initialized
            Ok(unsafe {
                core::mem::transmute::<&'a mut [MaybeUninit<T>], &'a mut [T]>(destination)
            })
        };

        // Safety: Closure error does not cause dangling pointers
        unsafe { self.alloc_try_with_layout(Layout::array::<T>(length).unwrap(), attempt) }
    }

    /// Allocates a slice to contain the items yielded by an iterator that may fail early.
    ///
    /// # Panics
    ///
    /// See the documentation for [`Bump::alloc_slice_from_iter`](Bump::alloc_slice_from_iter) for more information.
    fn alloc_slice_try_from_iter<T, E, I>(&'me self, items: I) -> Result<&'a mut [T], E>
    where
        I: IntoIterator<Item = Result<T, E>>,
        I::IntoIter: ExactSizeIterator,
    {
        let items_iter = items.into_iter();
        let layout = Layout::array::<T>(items_iter.len()).unwrap();
        let attempt = move |allocation: NonNull<u8>| -> Result<&'a mut [T], E> {
            // Safety: layout ensures length is valid, allocation is a valid pointer
            let destination = unsafe {
                core::slice::from_raw_parts_mut::<'a, _>(
                    allocation.cast::<MaybeUninit<T>>().as_ptr(),
                    items_iter.len(),
                )
            };

            let mut actual_length = 0usize;
            let mut destination_iter = destination.iter_mut();
            for value in items_iter {
                destination_iter
                    .next()
                    .expect("iterator yielded too many items")
                    .write(value?);
                actual_length += 1;
            }

            // If iterator "lies" and returns too few items, then slice
            let slice = &mut destination[0..actual_length];

            // Safety: [T] and [MaybeUninit<T>] have the same layout, slice is initialized
            Ok(unsafe { core::mem::transmute::<&'a mut [MaybeUninit<T>], &'a mut [T]>(slice) })
        };

        // Safety: Closure error does not cause dangling pointers
        unsafe { self.alloc_try_with_layout(layout, attempt) }
    }
}
