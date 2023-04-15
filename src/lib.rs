#![doc = include_str!("../README.md")]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![deny(unreachable_pub)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::alloc_instead_of_core)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod arena;
mod bump;
mod frame;
mod raw_arena;

pub use arena::Arena;
pub use bump::Bump;
pub use frame::Frame;

/// Imports commonly used types for bump allocation.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{Arena, Bump};
}
