//! Input and output streams for binary data.

use std::error::Error;
use std::io::{Read, Write};
use std::string::FromUtf8Error;
use thiserror::Error;
use crate::data::{Be, Le, PlainData};
use crate::context::Endian;

macro_rules! declare_type_enum {
    ($(#[$enum_meta:meta])*
    type enum $name:ident ($scope:ident) {
        $($(#[$meta:meta])* $variant:ident),+ $(,)?
    }) => {
        $(#[$enum_meta])*
        pub trait $name: sealed::Sealed {}
        mod sealed { pub trait Sealed {} }

        $(#[$enum_meta])*
        /// Provided for explicit scoping.
        pub mod $scope {
            $(
                $(#[$meta])*
                #[derive(Debug, Copy, Clone)]
                pub struct $variant;
                impl super::$name for $variant {}
                impl super::sealed::Sealed for $variant {}
            )+
        }
    }
}

declare_type_enum! {
    /// Direction tags: `Read` or `Write`.
    type enum Direction (dir) {
        /// Tag for the input direction.
        Read,
        /// Tag for the output direction.
        Write,
    }
}

/// Types that can be used as magic sequence.
pub trait IntoMagic {
    /// Representation of the magic sequence.
    type MagicRepr: Default + AsRef<[u8]> + AsMut<[u8]>;
    /// Convert into a magic sequence.
    fn into_magic(self) -> Self::MagicRepr;
}

impl IntoMagic for u8 {
    type MagicRepr = [u8; 1];
    fn into_magic(self) -> [u8; 1] { [self] }
}

impl<const N: usize> IntoMagic for [u8; N] where Self: Default {
    type MagicRepr = Self;
    fn into_magic(self) -> Self::MagicRepr { self }
}

impl<T: PlainData> IntoMagic for Le<T> {
    type MagicRepr = T::RawBytes;
    fn into_magic(self) -> Self::MagicRepr { self.0.to_bytes(Endian::Little) }
}
impl<T: PlainData> IntoMagic for Be<T> {
    type MagicRepr = T::RawBytes;
    fn into_magic(self) -> Self::MagicRepr { self.0.to_bytes(Endian::Big) }
}

/// Extensions shared by input and output streams.
pub trait Stream<Dir: Direction> {
    /// Error returned by stream operations.
    type StreamError: Error;
    /// Declares there is a magic sequence in the binary data.
    fn magic<M: IntoMagic>(&mut self, magic: M) -> Result<(), Self::StreamError>;
    /// Declares there is a padding in the binary data.
    fn pad(&mut self, n: usize) -> Result<(), Self::StreamError>;
}

/// Decoding errors.
#[derive(Debug, Error)]
pub enum DecodeError {
    /// Not enough bytes when decoding some data.
    #[error("incomplete '{0}': {1}")]
    IncompleteData(&'static str, std::io::Error),
    /// Invalid byte sequence for some data.
    #[error("invalid '{0}'")]
    InvalidData(&'static str),
    /// Incorrect magic number.
    #[error("incorrect magic: expecting '{expected_magic:?}', found '{real_bytes:?}'")]
    MagicMismatch {
        /// Real bytes in the binary file.
        real_bytes: Box<[u8]>,
        /// Expected magic byte sequence.
        expected_magic: Box<[u8]>,
    },
    /// Cannot decode UTF-8 strings.
    #[error("invalid UTF-8: found '{invalid_bytes:?}, after successfully decoding '{valid_prefix}''")]
    DecodeUtf8Error {
        /// The string is valid until this point.
        valid_prefix: Box<str>,
        /// The invalid bytes coming after the valid prefix.
        invalid_bytes: Box<[u8]>,
    },
    /// Superfluous bytes after decoding finished. EOF expected.
    #[error("input stream not exhausted, remaining bytes: {0:?}")]
    SuperfluousBytes(Box<[u8]>),
}

impl From<FromUtf8Error> for DecodeError {
    fn from(err: FromUtf8Error) -> Self {
        let utf8_error = err.utf8_error();
        let valid_up_to = utf8_error.valid_up_to();
        let invalid_to = valid_up_to + utf8_error.error_len().unwrap_or(0);
        let mut buffer = err.into_bytes();
        buffer.truncate(invalid_to);
        let invalid_bytes = buffer.split_off(valid_up_to).into_boxed_slice();
        let valid_prefix = String::from_utf8(buffer).unwrap().into_boxed_str();
        DecodeError::DecodeUtf8Error { valid_prefix, invalid_bytes }
    }
}

impl<R: Read + ?Sized> Stream<dir::Read> for R {
    type StreamError = DecodeError;
    fn magic<M: IntoMagic>(&mut self, magic: M) -> Result<(), DecodeError> {
        use DecodeError::IncompleteData;
        let mut buffer = M::MagicRepr::default();
        self.read_exact(buffer.as_mut()).map_err(|err| IncompleteData("magic", err))?;
        let expected = magic.into_magic();
        let expected = expected.as_ref();
        let actual = buffer.as_ref();
        if expected == actual { Ok(()) } else {
            Err(DecodeError::MagicMismatch {
                real_bytes: actual.into(),
                expected_magic: expected.into(),
            })
        }
    }
    fn pad(&mut self, n: usize) -> Result<(), DecodeError> {
        use DecodeError::IncompleteData;
        let mut buffer = vec![0_u8; n];
        self.read_exact(&mut buffer).map_err(|err| IncompleteData("padding", err))
    }
}

/// Encoding errors.
#[derive(Debug, Error)]
pub enum EncodeError {
    /// Invalid in-memory representation for some data.
    #[error("invalid argument for '{0}': {1}")]
    InvalidArgument(&'static str, &'static str),
    /// Invalid in-memory representation for some data.
    #[error("invalid '{0}'")]
    InvalidData(&'static str),
    /// I/O error in the output stream.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl<W: Write + ?Sized> Stream<dir::Write> for W {
    type StreamError = EncodeError;
    fn magic<M: IntoMagic>(&mut self, magic: M) -> Result<(), EncodeError> {
        let magic = magic.into_magic();
        self.write_all(magic.as_ref()).map_err(EncodeError::from)
    }
    fn pad(&mut self, n: usize) -> Result<(), EncodeError> {
        self.write_all(&vec![0_u8; n]).map_err(EncodeError::from)
    }
}
