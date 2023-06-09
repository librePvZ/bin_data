//! Declarative encoding and decoding for binary formats.

#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

pub mod context;
pub mod stream;
pub mod data;

#[cfg(feature = "macros")]
pub use bin_data_macros::bin_data;
