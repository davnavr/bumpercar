//! Contains types for bump allocation across threads.
//!
//! # Example
//!
//! ```
//! use bumpercar::{Bump, sync::SharedArena};
//! use rayon::prelude::*;
//!
//! let arena = SharedArena::new();
//! let my_nums = [1, 2, 3]
//!     .into_par_iter()
//!     .map_init(|| arena.allocator(), |allocator, i| {
//!         allocator.alloc(i)
//!     })
//!     .collect::<Vec<_>>();
//!
//! assert_eq!(*my_nums[2], 3);
//! ```

use crate::raw_arena::RawArena;
use std::alloc::Layout;
use std::ptr::NonNull;
use std::sync::Mutex;

/// A collection of [`Arena`](crate::Arena) instances shared between threads.
#[derive(Debug)]
pub struct SharedArena {
    arenas: Mutex<Vec<RawArena>>,
}

/// A bump allocator that allocates objects into a [`SharedArena`].
#[derive(Debug)]
pub struct ThreadAllocator<'a> {
    arena: RawArena,
    owner: &'a SharedArena,
}

// Safety: SharedArena lives for 'a, contains all arenas, and outlives 'me
unsafe impl<'me, 'a: 'me> crate::Bump<'me, 'a> for ThreadAllocator<'a> {
    #[inline(always)]
    fn alloc_with_layout(&'me self, layout: Layout) -> NonNull<u8> {
        self.arena.alloc_with_layout(layout)
    }

    #[inline(always)]
    unsafe fn realloc(
        &'me self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_size: usize,
    ) -> (NonNull<u8>, Option<NonNull<u8>>) {
        // Safety: ensured by caller
        unsafe { self.arena.realloc(pointer, old_layout, new_size) }
    }

    #[inline(always)]
    fn with_frame<T, F: FnOnce(&mut crate::Frame) -> T>(&'me mut self, f: F) -> T {
        crate::Frame::in_arena(&mut self.arena, f)
    }

    #[inline(always)]
    unsafe fn alloc_try_with_layout<R, F>(&'me self, layout: Layout, f: F) -> R
    where
        R: crate::private::Try,
        F: FnOnce(NonNull<u8>) -> R,
    {
        // Safety: ensured by caller
        unsafe { self.arena.alloc_try_with_layout(layout, f) }
    }
}

impl SharedArena {
    /// Creates a new empty [`SharedArena`].
    pub fn new() -> Self {
        Self {
            arenas: Default::default(),
        }
    }

    /// Marks the memory used by each [`Arena`](crate::Arena) as being freed.
    ///
    /// See [`Arena::reset()`](crate::Arena::reset) for more information.
    pub fn reset(&mut self) {
        for arena in self.arenas.get_mut().unwrap().iter_mut() {
            // Safety: &mut self ensures no extant references into arena
            unsafe {
                arena.reset();
            }
        }
    }

    /// Obtains a [`ThreadAllocator`] for use within the current thread.
    pub fn allocator(&self) -> ThreadAllocator<'_> {
        ThreadAllocator {
            arena: self.arenas.lock().unwrap().pop().unwrap_or_default(),
            owner: self,
        }
    }
}

impl Drop for ThreadAllocator<'_> {
    fn drop(&mut self) {
        // Don't want panic to occur, this will just leak memory if mutex was poisoned
        if let Ok(mut arenas) = self.owner.arenas.lock() {
            arenas.push(std::mem::take(&mut self.arena));
        }
    }
}

impl Default for SharedArena {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

// Safety: Safe to share, Mutex guards arenas and only hands them out to one thread at a time
unsafe impl Sync for SharedArena {}

// Safety: Safe to send across threads, data guarded by Mutex
unsafe impl Send for SharedArena {}

// Safety: Borrow checker ensures no dangling pointers if allocator is sent across threads
unsafe impl Send for ThreadAllocator<'_> {}
