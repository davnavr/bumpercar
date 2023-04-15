use crate::raw_arena::RawArena;
use core::ptr::NonNull;

/// An arena, owns regions of memory that objects are bump allocated into.
///
/// # Example
///
/// ```
/// use bumpercar::prelude::*;
///
/// let arena = Arena::new();
/// let my_num = arena.alloc(5i32);
/// assert_eq!(*my_num, 5i32);
///
/// arena.alloc("hello");
///
/// *my_num = 0xABCDi32;
/// assert_eq!(*my_num, 0xABCDi32);
/// ```
pub struct Arena {
    arena: RawArena,
}

impl Arena {
    /// Creates an empty [`Arena`].
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates an [`Arena`], allocating a new chunk to contain at least `capacity` bytes.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            arena: RawArena::with_capacity(capacity),
        }
    }
}

unsafe impl<'a> crate::Bump<'a, 'a> for Arena {
    fn alloc_with_layout(&'a self, layout: core::alloc::Layout) -> NonNull<u8> {
        self.arena.alloc_with_layout(layout)
    }
}

impl core::fmt::Debug for Arena {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Arena").finish_non_exhaustive()
    }
}

impl core::default::Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for Arena {}
