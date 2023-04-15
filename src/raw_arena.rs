use alloc::alloc;
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

const HEADER_SIZE: usize = std::mem::size_of::<ChunkHeader>();
const DEFAULT_CAPACITY: usize = 1024;

// Uses a "downward bumping allocator", see https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html

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
) -> Option<NonNull<ChunkHeader>> {
    let size = requested_capacity
        .get()
        .checked_next_power_of_two()?
        .checked_add(HEADER_SIZE)?;
    let layout = Layout::from_size_align(size, std::mem::align_of::<ChunkHeader>()).ok()?;
    let chunk = unsafe {
        // Safety: layout size is never 0
        let pointer = alloc::alloc(layout);
        let end = NonNull::new(pointer.add(layout.size()))?;

        // Safety: layout uses alignment of ChunkHeader, so reference is aligned
        let header = NonNull::new(pointer)?
            .cast::<MaybeUninit<ChunkHeader>>()
            .as_mut();
        Some(NonNull::from(header.write(ChunkHeader {
            previous: current_chunk.get(),
            end,
            finger: Cell::new(end),
        })))
    };

    current_chunk.set(chunk);
    chunk
}

pub(crate) struct RawArena {
    current_chunk: Cell<Option<NonNull<ChunkHeader>>>,
}

impl RawArena {}
