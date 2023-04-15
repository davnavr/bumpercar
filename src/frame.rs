use crate::raw_arena::RawArenaState;

/// Allows bump allocating and deallocating into a portion of an [`Arena`](crate::Arena).
pub struct Frame<'a> {
    /// The underlying arena where allocations occur
    ///
    /// This **must** never be exposed to the user, since at this point only a
    /// [`Frame`] should be calling allocation functions.
    arena: &'a mut crate::Arena,
    state: Option<RawArenaState>,
}

impl<'a> Frame<'a> {
    pub(crate) fn new(arena: &'a mut crate::Arena) -> Self {
        Self {
            state: arena.arena.current_state(),
            arena,
        }
    }

    pub(crate) unsafe fn restore(self) {
        // Safety: ensured by caller
        unsafe {
            self.arena.arena.restore_state(self.state);
        }
    }
}

// Safety: 'f is the lifetime of the frame, which is less than the lifetime of the arena 'a,
// so allocations live for the lifetime of the frame
unsafe impl<'a: 'f, 'f> crate::Bump<'f, 'f> for Frame<'a> {
    fn with_frame<T, F: FnOnce(&mut crate::Frame<'f>) -> T>(&'f mut self, f: F) -> T {
        self.arena.with_frame(f)
    }

    fn alloc_with_layout(&'f self, layout: core::alloc::Layout) -> core::ptr::NonNull<u8> {
        self.arena.alloc_with_layout(layout)
    }
}

impl core::fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Frame").field(&self.state).finish()
    }
}
