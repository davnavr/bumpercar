/// An arena, owns regions of memory that objects are bump allocated into.
///
/// To allocate objects into the arena, see the [`allocator()`](Arena::allocator) method.
#[derive(Debug)]
pub struct Arena {
    arena: crate::raw_arena::RawArena,
}

impl Arena {
    /// Creates an empty arena.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates an arena, allocating a new chunk to contain at least `capacity` bytes.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            arena: crate::raw_arena::RawArena::with_capacity(capacity),
        }
    }

    /// Returns an [`Allocator`] used to allocate objects into the arena.
    ///
    /// Note that the usage of `&mut self` ensure that **only** the returned [`Allocator`]
    /// can allocate objects into the arena.
    ///
    /// `Allocator`: crate::Allocator
    pub fn allocator(&mut self) -> crate::Allocator<'_> {
        crate::Allocator::with_arena(&mut self.arena)
    }

    /// Resets the arena by moving the bump pointer back to the first chunk.
    ///
    /// This allows reusing of memory allocated by the arena.
    pub fn reset(&mut self) {
        unsafe {
            // Safety: &mut self ensures there are no extant references that can become dangling
            self.arena.reset()
        }
    }
}

impl core::default::Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: Safe to send across threads, borrow checker ensures there are no extant Allocators
unsafe impl Send for Arena {}
