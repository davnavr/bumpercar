use alloc::alloc;
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

const HEADER_SIZE: usize = core::mem::size_of::<ChunkHeader>();
const CHUNK_ALIGNMENT: usize = 16;

const DEFAULT_CAPACITY: NonZeroUsize = {
    // Safety: not zero
    unsafe { NonZeroUsize::new_unchecked(1024) }
};

// Uses a "downward bumping allocator", see https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html

#[derive(Debug)]
#[non_exhaustive]
struct OutOfMemory;

impl core::fmt::Display for OutOfMemory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "out of memory")
    }
}

type Result<T> = core::result::Result<T, OutOfMemory>;

/// Allows for quick deallocation of a portion of a [`RawArena`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RawArenaState {
    chunk: NonNull<ChunkHeader>,
    finger: NonNull<u8>,
}

/// A memory chunk, the header is followed by the chunk's contents.
#[repr(C)]
struct ChunkHeader {
    /// Pointer to the previous chunk.
    previous: Option<NonNull<Self>>,
    /// Pointer to the next chunk.
    next: Cell<Option<NonNull<Self>>>,
    /// Pointer to the byte after the last byte of the chunk.
    end: NonNull<u8>,
    /// Points to the first byte of the region of the chunk's contents containing allocated
    /// objects.
    ///
    /// This must always be less than or equal to [`end`](Self::end) and greater than or equal to
    /// [`start`]. If this is equal to [`start`], then the chunk is full.
    ///
    /// [`start`]: Self::start
    finger: Cell<NonNull<u8>>,
    layout: Layout,
}

impl ChunkHeader {
    /// Pointer to the first byte of the chunk.
    ///
    /// The returned value is expected to be less than [`end`](Self::end).
    #[inline(always)]
    pub(crate) const fn start(&self) -> NonNull<u8> {
        // Safety: overflow will not occur, since allocation request would have failed
        unsafe { NonNull::new_unchecked((self as *const Self as *mut u8).add(HEADER_SIZE)) }
    }

    /// Returns the maximum amount, in bytes, of content that can be stored in this chunk.
    #[inline(always)]
    pub(crate) fn capacity(&self) -> NonZeroUsize {
        // Safety: size is never 0
        unsafe {
            NonZeroUsize::new_unchecked(self.end.as_ptr() as usize - self.start().as_ptr() as usize)
        }
    }

    #[inline(always)]
    fn fast_alloc_with_layout(&self, layout: Layout) -> Result<NonNull<u8>> {
        let start = self.start().as_ptr();
        let mut finger = self.finger.get().as_ptr();

        debug_assert!(finger >= start);
        debug_assert!((self as *const Self as *const u8) < finger);

        // This handles ZSTs correctly
        finger = finger.wrapping_sub(layout.size());
        finger = finger.wrapping_sub(finger as usize % layout.align());

        if finger >= start {
            debug_assert!(finger <= self.end.as_ptr());

            // Safety: finger is not null, since start is not null
            let finger = unsafe { NonNull::new_unchecked(finger) };

            self.finger.set(finger);
            Ok(finger)
        } else {
            Err(OutOfMemory)
        }
    }
}

fn get_next_or_allocate_chunk(
    current_chunk: &Cell<Option<NonNull<ChunkHeader>>>,
    default_capacity: Option<NonZeroUsize>,
) -> Result<NonNull<ChunkHeader>> {
    let previous_chunk = current_chunk.get();
    let previous_header = previous_chunk.map(|previous| {
        // Safety: previous pointer is valid.
        unsafe { previous.as_ref() }
    });

    if let Some(next_chunk) = previous_header.and_then(|chunk| chunk.next.get()) {
        // Safety: next pointer is valid
        let next_header = unsafe { next_chunk.as_ref() };

        // In cases where previous "states" are restored, a chunk may have some allocations remaining
        // This means that the returned chunk has to be set to an empty state
        next_header.finger.set(next_header.end);

        return Ok(next_chunk);
    }

    // Need to allocate a new chunk

    let size = previous_header
        .map(|chunk| chunk.capacity().get().checked_mul(2).unwrap_or(usize::MAX))
        .unwrap_or(default_capacity.unwrap_or(DEFAULT_CAPACITY).get())
        .checked_add(HEADER_SIZE)
        .ok_or(OutOfMemory)?;

    let rounded_size = size
        .checked_add(size % CHUNK_ALIGNMENT)
        .ok_or(OutOfMemory)?;

    let layout = Layout::from_size_align(rounded_size, CHUNK_ALIGNMENT).map_err(|_| OutOfMemory)?;

    let chunk = {
        let pointer;
        let end;

        // Safety: layout size is never 0
        unsafe {
            pointer = alloc::alloc(layout);
            end = NonNull::new(pointer.add(rounded_size)).ok_or(OutOfMemory)?;
        }

        let header;

        // Safety: layout uses alignment of ChunkHeader, so reference is aligned
        unsafe {
            header = NonNull::new(pointer)
                .ok_or(OutOfMemory)?
                .cast::<MaybeUninit<ChunkHeader>>()
                .as_mut();
        }

        NonNull::from(header.write(ChunkHeader {
            previous: previous_chunk,
            next: Cell::new(None),
            end,
            finger: Cell::new(end),
            layout,
        }))
    };

    if let Some(previous_chunk) = previous_header {
        debug_assert!(previous_chunk.next.get().is_none());
        previous_chunk.next.set(Some(chunk));
    }

    current_chunk.set(Some(chunk));
    Ok(chunk)
}

struct Chunks<'a> {
    current: Option<&'a ChunkHeader>,
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a ChunkHeader;

    fn next(&mut self) -> Option<&'a ChunkHeader> {
        let previous = self.current;
        self.current = self.current.and_then(|header| {
            header.previous.map(|previous| {
                // Safety: valid for lifetime 'a
                unsafe { previous.as_ref() }
            })
        });
        previous
    }
}

/// Low-level bump allocator that uses memory from the global allocator.
///
/// Users should ensure that they do not use pointers to allocated objects
/// after the arena has been dropped.
pub(crate) struct RawArena {
    current_chunk: Cell<Option<NonNull<ChunkHeader>>>,
}

impl RawArena {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        let arena = Self::default();

        if let actual_capacity @ Some(_) = NonZeroUsize::new(capacity) {
            get_next_or_allocate_chunk(&arena.current_chunk, actual_capacity).unwrap();
        }

        arena
    }

    #[inline(always)]
    pub(crate) fn alloc_with_layout(&self, layout: Layout) -> NonNull<u8> {
        match self.fast_alloc_with_layout(layout) {
            Ok(allocation) => allocation,
            Err(_) => self.slow_alloc_with_layout(layout).unwrap(),
        }
    }

    #[inline(always)]
    fn fast_alloc_with_layout(&self, layout: Layout) -> Result<NonNull<u8>> {
        if let Some(chunk) = self.current_chunk.get() {
            // Safety: chunk is valid reference
            unsafe { chunk.as_ref() }.fast_alloc_with_layout(layout)
        } else {
            Err(OutOfMemory)
        }
    }

    #[inline(never)]
    fn slow_alloc_with_layout(&self, layout: Layout) -> Result<NonNull<u8>> {
        let chunk = if let Some(chunk) = self.current_chunk.get() {
            chunk
        } else {
            get_next_or_allocate_chunk(&self.current_chunk, None)?
        };

        // Safety: chunk is valid reference
        unsafe { chunk.as_ref() }.fast_alloc_with_layout(layout)
    }

    pub(crate) unsafe fn alloc_try_with_layout<R, F>(&self, layout: Layout, f: F) -> R
    where
        R: crate::private::Try,
        F: FnOnce(NonNull<u8>) -> R,
    {
        let rewind_state = self.current_state();
        let allocation = self.alloc_with_layout(layout);
        let success_state = self.current_state();

        // No UB if panic occurs here
        let result = f(allocation);

        // Check that state has not changed, closure may have called arena methods which may lead
        // to dangling pointers
        if !result.is_success() && self.current_state() == success_state {
            // Safety: calls are nested correctly
            unsafe {
                self.restore_state(rewind_state);
            }
        }

        result
    }

    /// Returns an [`ArenaState`], a snapshot of the state of this arena's chunks.
    pub(crate) fn current_state(&self) -> Option<RawArenaState> {
        self.current_chunk.get().map(|chunk| {
            // Safety: chunk is a valid pointer
            let header = unsafe { chunk.as_ref() };

            RawArenaState {
                chunk,
                finger: header.finger.get(),
            }
        })
    }

    /// Restores an earlier [`ArenaState`].
    ///
    /// # Safety
    ///
    /// The [`ArenaState`] passed as a parameter **must** have been returned by an earlier call
    /// from [`current_state`] for `self`.
    ///
    /// In between the [`current_state`] call used to obtain the [`ArenaState`] passed as a parameter
    /// and the call to [`restore_state`]:
    /// - The specific [`ArenaState`] parameter must not have been passed to any other call to
    ///   [`restore_state`]
    /// - Any other calls to [`restore_state`] must be paired with calls to [`current_state`] in the same interval.
    ///
    /// Essentially, calls to restore_state make an implicit stack.
    ///
    /// ```text
    /// A = current state #1
    /// B = current state #2
    /// C = current state #3
    /// ...
    /// restore state C
    /// restore state B
    /// restore state A
    /// ```
    ///
    /// `current_state`: Self::current_state
    /// `restore_state`: Self::restore_state
    pub(crate) unsafe fn restore_state(&self, state: Option<RawArenaState>) {
        if let Some(restoring) = state {
            self.current_chunk.set(Some(restoring.chunk));

            // Safety: chunk is valid for self
            let chunk = unsafe { restoring.chunk.as_ref() };

            chunk.finger.set(restoring.finger);
        } else {
            // Safety: requirements for this function are stricter than reset()
            unsafe { self.reset() }
        }
    }

    fn chunks(&self) -> Chunks<'_> {
        Chunks {
            current: self.current_chunk.get().map(|chunk| {
                // Safety: valid for lifetime of self
                unsafe { chunk.as_ref() }
            }),
        }
    }

    pub(crate) unsafe fn reset(&self) {
        for header in self.chunks() {
            header.finger.set(header.end);
        }
    }
}

impl Default for RawArena {
    #[inline(always)]
    fn default() -> Self {
        Self {
            current_chunk: Cell::new(None),
        }
    }
}

impl Drop for RawArena {
    fn drop(&mut self) {
        for header in self.chunks() {
            // Safety: pointer to chunk is valid, layout is the same
            unsafe { alloc::dealloc(header as *const ChunkHeader as *mut u8, header.layout) }
        }
    }
}

impl core::fmt::Debug for RawArena {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RawArena").finish_non_exhaustive()
    }
}

#[cfg(any(test, miri))]
mod tests {
    use super::RawArena;
    use core::alloc::Layout;

    #[test]
    fn simple_allocate_and_free() {
        let arena = RawArena::with_capacity(0);
        arena.alloc_with_layout(Layout::new::<i64>());
        core::mem::drop(arena);
    }
}
