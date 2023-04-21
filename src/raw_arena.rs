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

impl From<core::alloc::LayoutError> for OutOfMemory {
    fn from(_: core::alloc::LayoutError) -> Self {
        Self
    }
}

type Result<T> = core::result::Result<T, OutOfMemory>;

enum ReallocKind {
    Shrink,
    Grow,
}

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
    previous: Cell<Option<NonNull<Self>>>,
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
    ///// Counter used to keep track of the amount of free bytes in this chunk and subsequent chunks.
    //capacity: Cell<usize>,
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

    ///// Returns the remaining number of bytes in this chunk.
    //#[inline(always)]
    //pub(crate) fn size(&self) -> usize {
    //    self.finger.get().as_ptr() as usize - self.start().as_ptr() as usize
    //}

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

    unsafe fn fast_realloc(
        &self,
        pointer: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<(NonNull<u8>, ReallocKind)> {
        let mut finger = self.finger.get().as_ptr();

        // In these cases, alloc should be used instead
        if finger != pointer.as_ptr() || layout.size() == 0 {
            return Err(OutOfMemory);
        }

        debug_assert!(
            finger as usize % layout.align() == 0,
            "original allocation was aligned"
        );

        let start = self.start();

        let aligned_layout = layout.pad_to_align();
        let new_layout = Layout::from_size_align(new_size, layout.align())?.pad_to_align();

        // Adjust finger to make/reduce space for object
        let kind: ReallocKind;
        let amount;
        if aligned_layout.size() >= new_layout.size() {
            kind = ReallocKind::Shrink;
            amount = aligned_layout.size() - new_layout.size();
            finger = finger.wrapping_add(amount);
            debug_assert!(finger >= start.as_ptr());
        } else {
            // aligned_layout.size() < new_layout.size()
            kind = ReallocKind::Grow;
            amount = new_layout.size() - aligned_layout.size();
            finger = finger.wrapping_sub(amount);

            // Only failure state here is if chunk does not have space for new size
            if finger < start.as_ptr() {
                return Err(OutOfMemory);
            }
        }

        let allocation = NonNull::new(finger).ok_or(OutOfMemory)?;
        debug_assert!(allocation <= self.end);

        // Common code, a move still needs to occur whether a shrink or grow occured.
        // Note that if new_size == 0, then a "free" basically occured, which is still handled here
        if amount >= new_size {
            // Safety: If shrinking/growing by amount, then ranges are not overlapping
            unsafe { core::ptr::copy_nonoverlapping(pointer.as_ptr(), finger, new_size) }
        } else {
            // Safety: proper alignment & sizes
            unsafe { core::ptr::copy(pointer.as_ptr(), finger, new_size) }
        }

        self.finger.set(allocation);
        Ok((allocation, kind))
    }
}

fn get_next_or_allocate_chunk(
    current: &Cell<Option<NonNull<ChunkHeader>>>,
    default_capacity: Option<NonZeroUsize>,
    allocation_request: Option<NonZeroUsize>,
) -> Result<NonNull<ChunkHeader>> {
    let previous_chunk = current.get();
    let previous_header = previous_chunk.map(|previous| {
        // Safety: previous pointer is valid.
        unsafe { previous.as_ref() }
    });

    let mut next_chunk = previous_header.and_then(|chunk| chunk.next.get());
    let mut next_header = next_chunk.map(|next| {
        // Safety: next pointer is valid.
        unsafe { next.as_ref() }
    });

    let mut size: usize;
    let old_next: Option<&ChunkHeader>;
    if let Some(next) = next_header {
        debug_assert_eq!(next.previous.get(), previous_chunk);

        match allocation_request {
            Some(request) if request < next.capacity() => {
                // Special case, existing chunk is too small so reallocation must occur.

                size = HEADER_SIZE.checked_add(request.get()).ok_or(OutOfMemory)?;

                // Go to normal allocation path
                old_next = next.next.get().map(|next| {
                    // Safety: next pointer is valid
                    unsafe { next.as_ref() }
                });
            }
            _ => {
                // In cases where previous "states" are restored, a chunk may have some allocations remaining
                // This means that the returned chunk has to be set to an empty state
                next.finger.set(next.end);

                current.set(next_chunk);
                return Ok(NonNull::from(next));
            }
        }
    } else {
        // Need to allocate a new chunk

        size = previous_header
            .map(|chunk| chunk.capacity().get().checked_mul(2).unwrap_or(usize::MAX))
            .unwrap_or(default_capacity.unwrap_or(DEFAULT_CAPACITY).get())
            .checked_add(HEADER_SIZE)
            .ok_or(OutOfMemory)?;

        // If an alloc request was made that is greater than capacity * 2, need to adjust size so new
        // chunk will contain the request
        if let Some(request_size) = allocation_request {
            let content_size = size - HEADER_SIZE; // Does not underflow, >= HEADER_SIZE
            if content_size < request_size.get() {
                size = size
                    .checked_add(request_size.get() - content_size)
                    .ok_or(OutOfMemory)?;
            }
        }

        old_next = None;
    }

    let rounded_size = size
        .checked_add(size % CHUNK_ALIGNMENT)
        .ok_or(OutOfMemory)?;

    let layout = Layout::from_size_align(rounded_size, CHUNK_ALIGNMENT)?;

    let chunk = {
        let pointer;
        let end;

        // Safety: layout size is never 0
        unsafe {
            // TODO: Choose whether to alloc or realloc
            if let Some(next) = next_header {
                pointer = alloc::realloc(
                    next as *const ChunkHeader as *mut u8,
                    next.layout,
                    layout.size(),
                );

                // Prevents accidental further usage of dangling next pointer
                #[allow(unused_assignments)]
                {
                    next_header = None;
                    next_chunk = None;
                }
            } else {
                pointer = alloc::alloc(layout);
            }

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
            previous: Cell::new(previous_chunk),
            next: Cell::new(old_next.map(NonNull::from)),
            end,
            finger: Cell::new(end),
            layout,
        }))
    };

    if let Some(previous) = previous_header {
        debug_assert!(previous.next.get().is_none());
        previous.next.set(Some(chunk));
    }

    if let Some(old_header) = old_next {
        // Path only taken if a chunk was "reallocated" to fit a large allocation, need to fix a dangling pointer
        old_header.previous.set(Some(chunk));
    }

    current.set(Some(chunk));
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
            header.previous.get().map(|previous| {
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
            get_next_or_allocate_chunk(&arena.current_chunk, actual_capacity, None).unwrap();
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
        let chunk = get_next_or_allocate_chunk(
            &self.current_chunk,
            None,
            NonZeroUsize::new(layout.size()),
        )?;

        // Safety: chunk is valid reference
        unsafe { chunk.as_ref() }.fast_alloc_with_layout(layout)
    }

    pub(crate) unsafe fn realloc(
        &self,
        pointer: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> (NonNull<u8>, Option<NonNull<u8>>) {
        // Safety: ensured by caller
        let fast = unsafe { self.realloc_fast(pointer, layout, new_size) };
        match fast {
            Ok((adjusted, ReallocKind::Shrink)) => (adjusted, Some(pointer)),
            Ok((adjusted, ReallocKind::Grow)) => (adjusted, None),
            Err(_) => {
                // Safety: ensured by caller
                let slow = unsafe { self.realloc_slow(new_size, layout.align()) };
                (slow, Some(pointer))
            }
        }
    }

    #[inline(always)]
    unsafe fn realloc_fast(
        &self,
        pointer: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<(NonNull<u8>, ReallocKind)> {
        if let Some(chunk) = self.current_chunk.get() {
            // Safety: chunk is valid reference
            // Safety: caller ensures realloc safety
            unsafe { chunk.as_ref().fast_realloc(pointer, layout, new_size) }
        } else {
            Err(OutOfMemory)
        }
    }

    #[inline(never)]
    unsafe fn realloc_slow(&self, new_size: usize, old_alignment: usize) -> NonNull<u8> {
        self.alloc_with_layout(Layout::from_size_align(new_size, old_alignment).unwrap())
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
    /// This function is **very** unsafe, and can easily cause dangling pointers.
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
    /// [`current_state`]: Self::current_state
    /// [`restore_state`]: Self::restore_state
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
