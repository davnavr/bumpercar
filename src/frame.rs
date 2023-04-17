use crate::raw_arena::{RawArena, RawArenaState};

/// A bump allocator that allocates objects into a portion of an [`Arena`](crate::Arena).
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
///     frame.with_frame(|frame| {
///         let c = frame.alloc(*b * 2);
///         println!("{c}");
///     });
/// });
/// ```
///
/// ```compile_fail
/// let mut allocator = arena.allocator();
/// allocator.with_frame(|mut frame| {
///     // Does not compile, cannot return reference to object in frame since it is deallocated
///     frame.alloc(32);
/// });
/// ```
/// 
/// ```compile_fail
/// let mut allocator = arena.allocator();
/// allocator.with_frame(|mut frame| {
///     // Does not compile, frame must be used for allocations
///     let _ = allocator.alloc(42);
/// });
/// ```
#[derive(Debug)]
pub struct Frame<'a: 'f, 'f> {
    arena: &'f mut &'a mut RawArena,
}

impl<'a: 'f, 'f> Frame<'a, 'f> {
    pub(crate) fn in_arena<T, F: FnOnce(&mut Frame<'a, '_>) -> T>(mut arena: &'a mut RawArena, f: F) -> T {
        let state: Option<RawArenaState> = arena.current_state();
        let mut frame = Frame::<'a, '_> { arena: &mut arena };

        // If a panic occurs, then bump pointer does not get adjusted back
        // Only problem is unused memory (memory leak), which is not unsafe or UB
        let result = f(&mut frame);

        // Safety: calls are nested correctly
        unsafe {
            arena.restore_state(state);
        }

        result
    }
}

// Safety: 'f is the lifetime of the frame, which is less than the lifetime of the arena 'a,
// so allocations live for the lifetime of the frame
unsafe impl<'a: 'f, 'f: 'me, 'me> crate::Bump<'me, 'f> for Frame<'a, 'f> {
    #[inline]
    fn with_frame<T, F: FnOnce(&mut Frame) -> T>(&'me mut self, f: F) -> T {
        Frame::in_arena::<T, F>(self.arena, f)
    }

    #[inline]
    fn alloc_with_layout(&'me self, layout: core::alloc::Layout) -> core::ptr::NonNull<u8> {
        self.arena.alloc_with_layout(layout)
    }
}
