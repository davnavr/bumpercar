use crate::raw_arena::RawArena;
use core::alloc::Layout;
use core::ptr::NonNull;

/// A bump allocator that allocates objects into an [`Arena`](crate::Arena).
///
/// # Example
///
/// ```
/// use bumpercar::prelude::*;
///
/// let mut arena = Arena::new();
/// let allocator = arena.allocator();
/// let my_num = allocator.alloc(5i32);
/// assert_eq!(*my_num, 5i32);
///
/// allocator.alloc("hello");
///
/// *my_num = 0xABCDi32;
/// assert_eq!(*my_num, 0xABCDi32);
/// ```
#[derive(Debug)]
pub struct Allocator<'a> {
    arena: &'a mut RawArena,
}

impl<'a> Allocator<'a> {
    /// Creates a new [`Allocator`] to allocate into the specified arena.
    ///
    /// See [`Arena::allocator()`](crate::Arena::allocator) for information regarding the
    /// usage of a mutable reference.
    pub(crate) fn with_arena(arena: &'a mut RawArena) -> Self {
        Self { arena }
    }
}

// Safety: Allocator 'me lives as long as the arena 'a
unsafe impl<'me, 'a: 'me> crate::Bump<'me, 'a> for Allocator<'a> {
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
        crate::Frame::in_arena(self.arena, f)
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
