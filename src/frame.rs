use crate::raw_arena::{RawArena, RawArenaState};

/// Allows bump allocating and deallocating into a portion of an [`Arena`](crate::Arena).
///
/// # Examples
///
/// ```
/// use bumpercar::prelude::*;
///
/// let mut arena = Arena::new();
/// let mut allocator = arena.allocator();
/// let a = allocator.alloc(64);
/// allocator.with_frame(|mut frame| {
///     // Previous allocations can be used, but new allocations have to use the frame
///     let b = frame.alloc(*a - 32);
///     println!("Hello {b}!");
///
///     // This is a compile error, the frame has to be used to allocate new objects
///     //allocator.alloc(5);
///
///     frame.with_frame(|frame| {
///         let c = frame.alloc(*b * 2);
///         println!("{c}");
///     });
/// });
/// ```
pub struct Frame<'a> {
    arena: &'a mut RawArena,
    state: Option<RawArenaState>,
}

impl<'a> Frame<'a> {
    pub(crate) fn with_arena(arena: &'a mut RawArena) -> Self {
        Self {
            state: arena.current_state(),
            arena,
        }
    }

    pub(crate) unsafe fn restore(self) {
        // Safety: ensured by caller
        unsafe {
            self.arena.restore_state(self.state);
        }
    }

    pub(crate) fn in_arena<'f, T, F: FnOnce(&mut Frame<'f>) -> T>(
        arena: &'a mut RawArena,
        f: F,
    ) -> T
    where
        'a: 'f,
    {
        let mut frame = Frame::with_arena(arena);

        // If a panic occurs, then bump pointer does not get adjusted back
        // Only problem is unused memory (memory leak), which is not unsafe or UB
        let result = f(&mut frame);

        // Safety: calls are nested correctly
        unsafe {
            frame.restore();
        }

        result
    }
}

// Safety: 'f is the lifetime of the frame, which is less than the lifetime of the arena 'a,
// so allocations live for the lifetime of the frame
unsafe impl<'a: 'f, 'f> crate::Bump<'f, 'f> for Frame<'a> {
    // TODO: Have a 'me lifetime?
    #[inline]
    fn with_frame<T, F: FnOnce(&mut crate::Frame<'f>) -> T>(&'f mut self, f: F) -> T {
        Frame::in_arena::<'f, T, F>(self.arena, f)
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
