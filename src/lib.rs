#![doc = include_str!("../README.md")]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![deny(unreachable_pub)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::alloc_instead_of_core)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod bump;

pub use bump::Bump;

/// Imports commonly used types for bump allocation.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::Bump;
}
