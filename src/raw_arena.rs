use alloc::alloc;
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

const HEADER_SIZE: usize = std::mem::size_of::<ChunkHeader>();
const DEFAULT_CAPACITY: usize = 1024;

// Uses a "downward bumping allocator", see https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct OutOfMemory;

impl core::fmt::Display for OutOfMemory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "out of memory")
    }
}

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
    /// This must always be less than or equal to [`end`]. If this is equal to [`end`],
    /// then the chunk is completely full.
    ///
    /// [`end`]: Self::end
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
}

fn allocate_chunk(
    current_chunk: &Cell<Option<NonNull<ChunkHeader>>>,
    requested_capacity: NonZeroUsize,
) -> Result<(), OutOfMemory> {
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

    unsafe {
        // Safety: layout size is never 0
        let pointer = alloc::alloc(layout);
        let end = NonNull::new(pointer.add(layout.size())).ok_or(OutOfMemory)?;

        // Safety: layout uses alignment of ChunkHeader, so reference is aligned
        let header = NonNull::new(pointer)
            .ok_or(OutOfMemory)?
            .cast::<MaybeUninit<ChunkHeader>>()
            .as_mut();

        current_chunk.set(Some(NonNull::from(header.write(ChunkHeader {
            previous: current_chunk.get(),
            end,
            finger: Cell::new(end),
            previous_allocation_size: Cell::new(None),
        }))));
    }

    Ok(())
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
}
