//! Contains types for bump allocation across threads.

use crate::raw_arena::RawArena;
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
    fn alloc_with_layout(&'me self, layout: core::alloc::Layout) -> core::ptr::NonNull<u8> {
        self.arena.alloc_with_layout(layout)
    }

    #[inline(always)]
    fn with_frame<T, F: FnOnce(&mut crate::Frame<'me>) -> T>(&'me mut self, f: F) -> T {
        crate::Frame::in_arena(&mut self.arena, f)
    }
}

impl SharedArena {
    /// Creates a new empty [`SharedArena`].
    pub fn new() -> Self {
        Self {
            arenas: Default::default(),
        }
    }

    /// Marks the memory used by each [`Arena`] as being freed.
    ///
    /// See [`Arena::reset()`](Arena::reset) for more information.
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
