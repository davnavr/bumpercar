use alloc::alloc;
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

const HEADER_SIZE: usize = std::mem::size_of::<ChunkHeader>();

const DEFAULT_CAPACITY: NonZeroUsize = unsafe {
    // Safety: not zero
    NonZeroUsize::new_unchecked(1024)
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

/// A memory chunk, the header is followed by the chunk's contents.
#[repr(C)]
struct ChunkHeader {
    /// Pointer to the previous chunk.
    previous: Option<NonNull<Self>>,
    /// Pointer to the last byte of the chunk.
    end: NonNull<u8>,
    /// Points to the first byte of the region of the chunk's contents containing allocated
    /// objects.
    ///
    /// This must always be less than or equal to [`end`](Self::end) and greater than or equal to
    /// [`start`]. If this is equal to [`start`], then the chunk is full.
    ///
    /// [`start`]: Self::start
    finger: Cell<NonNull<u8>>,
    /// The size, in bytes, of the last memory allocation made in this chunk.
    previous_allocation_size: Cell<Option<NonZeroUsize>>,
}

impl ChunkHeader {
    /// Pointer to the first byte of the chunk.
    ///
    /// The returned value is expected to be less than [`end`](Self::end).
    #[inline(always)]
    pub(crate) const fn start(&self) -> NonNull<u8> {
        unsafe {
            // Safety: overflow will not occur, since allocation request would have failed
            NonNull::new_unchecked((self as *const Self as *mut u8).add(HEADER_SIZE))
        }
    }

    /// Returns the size of the chunk.
    #[inline(always)]
    pub(crate) fn size(&self) -> NonZeroUsize {
        unsafe {
            // Safety: size is never 0
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
            Ok(unsafe {
                // Safety: finger is not null, since start is not null
                NonNull::new_unchecked(finger)
            })
        } else {
            Err(OutOfMemory)
        }
    }
}

fn allocate_chunk(
    current_chunk: &Cell<Option<NonNull<ChunkHeader>>>,
    requested_capacity: NonZeroUsize,
) -> Result<NonNull<ChunkHeader>> {
    let size = requested_capacity
        .checked_next_power_of_two()
        .ok_or(OutOfMemory)?
        .checked_add(HEADER_SIZE)
        .ok_or(OutOfMemory)?;

    let layout = Layout::from_size_align(
        size.get(),
        core::cmp::max(std::mem::align_of::<ChunkHeader>(), 16),
    )
    .map_err(|_| OutOfMemory)?;

    let chunk = unsafe {
        // Safety: layout size is never 0
        let pointer = alloc::alloc(layout);
        let end = NonNull::new(pointer.add(layout.size())).ok_or(OutOfMemory)?;

        // Safety: layout uses alignment of ChunkHeader, so reference is aligned
        let header = NonNull::new(pointer)
            .ok_or(OutOfMemory)?
            .cast::<MaybeUninit<ChunkHeader>>()
            .as_mut();

        NonNull::from(header.write(ChunkHeader {
            previous: current_chunk.get(),
            end,
            finger: Cell::new(end),
            previous_allocation_size: Cell::new(None),
        }))
    };

    current_chunk.set(Some(chunk));
    Ok(chunk)
}

pub(crate) struct RawArena {
    current_chunk: Cell<Option<NonNull<ChunkHeader>>>,
}

impl RawArena {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        let arena = Self {
            current_chunk: Cell::new(None),
        };

        if let Some(capacity) = NonZeroUsize::new(capacity) {
            allocate_chunk(&arena.current_chunk, capacity).unwrap();
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
            unsafe {
                // Safety: chunk is valid reference
                chunk.as_ref()
            }
            .fast_alloc_with_layout(layout)
        } else {
            Err(OutOfMemory)
        }
    }

    #[inline(never)]
    fn slow_alloc_with_layout(&self, layout: Layout) -> Result<NonNull<u8>> {
        let chunk = if let Some(chunk) = self.current_chunk.get() {
            chunk
        } else {
            allocate_chunk(&self.current_chunk, DEFAULT_CAPACITY)?
        };

        unsafe {
            // Safety: chunk is valid reference
            chunk.as_ref()
        }
        .fast_alloc_with_layout(layout)
    }
}
