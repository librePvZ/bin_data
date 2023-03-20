//! Interface for encoding and decoding binary data.

use std::io::{Read, Write};
use std::ops::Deref;
use crate::named_args::{ArgsBuilderFinished, Endian, EndianBuilder, InheritEndian, NamedArgs, Provided, Required, VecArgs, VecArgsBuilder};
use crate::stream::{dir, DecodeError, Direction, EncodeError};

/// Decode binary data to structured in-memory representation.
pub trait Decode<Args = ()>: NamedArgs<dir::Read> + Sized {
    /// Decode an instance of `Self` from input stream with the given arguments.
    fn decode_with<R: Read + ?Sized>(reader: &mut R, args: Args) -> Result<Self, DecodeError>;
    /// Decode an instance of `Self` from input stream with default arguments.
    fn decode<R: Read + ?Sized>(reader: &mut R) -> Result<Self, DecodeError>
        where Self::ArgsBuilder: ArgsBuilderFinished<Output = Args> {
        Self::decode_with(reader, Self::args_builder().finish())
    }
}

/// Encode binary data from structured in-memory representation.
pub trait Encode<Args = ()>: NamedArgs<dir::Write> {
    /// Encode `self` to the output stream with the given arguments.
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, args: Args) -> Result<(), EncodeError>;
    /// Encode `self` to the output stream with default arguments.
    fn encode<W: Write + ?Sized>(&self, writer: &mut W) -> Result<(), EncodeError>
        where Self::ArgsBuilder: ArgsBuilderFinished<Output = Args> {
        self.encode_with(writer, Self::args_builder().finish())
    }
}

impl<T: NamedArgs<dir::Write> + ?Sized> NamedArgs<dir::Write> for Box<T> {
    type ArgsBuilder = T::ArgsBuilder;
    fn args_builder() -> Self::ArgsBuilder { T::args_builder() }
}

impl<Args, T: Encode<Args> + ?Sized> Encode<Args> for Box<T> {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, args: Args) -> Result<(), EncodeError> {
        self.deref().encode_with(writer, args)
    }
}

/// Marker trait: `U: View<T>` indicates that when we need to encode a value of type `T`, we can
/// encode a value of `U` instead.
pub trait View<T: ?Sized> {}

impl<T: ?Sized> View<T> for &'_ T {}
impl<T: ?Sized> View<Box<T>> for &'_ T {}
impl<T> View<Vec<T>> for &'_ [T] {}

/// View into a slice, with every element projected using `P`.
#[derive(Debug, Copy, Clone)]
pub struct SliceView<'a, T, P> {
    base_slice: &'a [T],
    projector: P,
}

// the `Fn` bound for `P` should help type inference
impl<'a, T, U: 'a, P: Fn(&T) -> &U> SliceView<'a, T, P> {
    /// Create a new view into the slice.
    pub fn new(base_slice: &'a [T], projector: P) -> Self {
        SliceView { base_slice, projector }
    }
}

impl<'a, T, U: 'a, P: Fn(&T) -> &U> View<Vec<U>> for SliceView<'a, T, P> {}
impl<'a, T, U: 'a, P: Fn(&T) -> &U> View<Box<[U]>> for SliceView<'a, T, P> {}

impl<'a, T, U: 'a, P: Fn(&T) -> &U> IntoIterator for SliceView<'a, T, P> {
    type Item = &'a U;
    type IntoIter = std::iter::Map<std::slice::Iter<'a, T>, P>;
    fn into_iter(self) -> Self::IntoIter { self.base_slice.iter().map(self.projector) }
}

impl<'a, 'b, T, U: 'a, P: Fn(&T) -> &U> IntoIterator for &'b SliceView<'a, T, P> {
    type Item = &'b U;
    type IntoIter = std::iter::Map<std::slice::Iter<'b, T>, &'b P>;
    fn into_iter(self) -> Self::IntoIter { self.base_slice.iter().map(&self.projector) }
}

impl<'a, A, B: 'a, P: Fn(&A) -> &B> NamedArgs<dir::Write> for SliceView<'a, A, P> {
    type ArgsBuilder = VecArgsBuilder<Provided<std::iter::Repeat<()>>, B, fn(&B) -> &B>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<'a, A, B: 'a, P: Fn(&A) -> &B, Args, U, F> Encode<VecArgs<Args, B, F>> for SliceView<'a, A, P>
    where Args: Iterator, U: Encode<Args::Item>, F: Fn(&B) -> &U {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, args: VecArgs<Args, B, F>) -> Result<(), EncodeError> {
        let mut element_args = args.element_args;
        self.into_iter().map(&args.transform).try_for_each(|x| {
            const ERR: EncodeError = EncodeError::InvalidArgument("Vec", "not enough arguments");
            let arg = element_args.next().ok_or(ERR)?;
            x.encode_with(writer, arg)
        })
    }
}

/// Used in automatically generated code to aid type inference.
pub fn assert_is_view<T: ?Sized, U: View<T>>(x: U) -> U { x }

/// Plain old data, can be directly encoded to and decoded from raw bytes.
pub trait PlainData: Sized {
    /// Storage type for the raw bytes, typically a `[u8; N]`.
    type RawBytes: Default + AsMut<[u8]> + AsRef<[u8]>;
    /// Convert from raw bytes to an instance of `Self`.
    fn from_bytes(bytes: Self::RawBytes, endian: Endian) -> Self;
    /// Convert `self` to its raw bytes.
    fn to_bytes(&self, endian: Endian) -> Self::RawBytes;
}

macro_rules! impl_primitive_plain_data {
    ($($t:ty),+ $(,)?) => {
        $(
            impl PlainData for $t {
                type RawBytes = [u8; std::mem::size_of::<Self>()];
                fn from_bytes(bytes: Self::RawBytes, endian: Endian) -> Self {
                    match endian {
                        Endian::Little => Self::from_le_bytes(bytes),
                        Endian::Big => Self::from_be_bytes(bytes),
                    }
                }
                fn to_bytes(&self, endian: Endian) -> Self::RawBytes {
                    match endian {
                        Endian::Little => self.to_le_bytes(),
                        Endian::Big => self.to_be_bytes(),
                    }
                }
            }

            impl View<$t> for $t {}

            impl<Dir: Direction> NamedArgs<Dir> for $t {
                type ArgsBuilder = EndianBuilder<Required>;
                fn args_builder() -> Self::ArgsBuilder { EndianBuilder::default() }
            }

            impl Decode<Endian> for $t {
                fn decode_with<R: Read + ?Sized>(reader: &mut R, endian: Endian) -> Result<Self, DecodeError> {
                    plain_data_decode_with(reader, endian)
                }
            }

            impl Encode<Endian> for $t {
                fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Endian) -> Result<(), EncodeError> {
                    plain_data_encode_with(self, writer, endian)
                }
            }
        )+
    }
}

// it is a pity that `Box<T>` might actually be implemented as `PlainData`
// so it conflicts with `impl<T: Encode> Encode for Box<T>`
// to work around this, we break the blanket impl on `T: PlainData` to standalone impls
impl_primitive_plain_data! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
    f32, f64,
}

fn plain_data_decode_with<T: PlainData, R: Read + ?Sized>(
    reader: &mut R, endian: Endian,
) -> Result<T, DecodeError> {
    use DecodeError::IncompleteData;
    let t_name = std::any::type_name::<T>();
    let mut buffer = T::RawBytes::default();
    reader.read_exact(buffer.as_mut()).map_err(|err| IncompleteData(t_name, err))?;
    Ok(T::from_bytes(buffer, endian))
}

fn plain_data_encode_with<T: PlainData, W: Write + ?Sized>(
    value: &T, writer: &mut W, endian: Endian,
) -> Result<(), EncodeError> {
    writer.write_all(value.to_bytes(endian).as_ref()).map_err(EncodeError::from)
}

/// Wrapper for little-endian data.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Le<T>(pub T);

impl<Dir: Direction, T: NamedArgs<Dir>> NamedArgs<Dir> for Le<T>
    where T::ArgsBuilder: InheritEndian {
    type ArgsBuilder = <T::ArgsBuilder as InheritEndian>::WithEndian;
    fn args_builder() -> Self::ArgsBuilder {
        T::args_builder().inherit_endian(Endian::Little)
    }
}

impl<Args, T: Decode<Args>> Decode<Args> for Le<T>
    where T::ArgsBuilder: InheritEndian {
    fn decode_with<R: Read + ?Sized>(reader: &mut R, args: Args) -> Result<Self, DecodeError> {
        T::decode_with(reader, args).map(Le)
    }
}

impl<Args, T: Encode<Args>> Encode<Args> for Le<T>
    where T::ArgsBuilder: InheritEndian {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, args: Args) -> Result<(), EncodeError> {
        self.0.encode_with(writer, args)
    }
}

/// Wrapper for big-endian data.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Be<T>(pub T);

impl<Dir: Direction, T: NamedArgs<Dir>> NamedArgs<Dir> for Be<T>
    where T::ArgsBuilder: InheritEndian {
    type ArgsBuilder = <T::ArgsBuilder as InheritEndian>::WithEndian;
    fn args_builder() -> Self::ArgsBuilder {
        T::args_builder().inherit_endian(Endian::Big)
    }
}

impl<Args, T: Decode<Args>> Decode<Args> for Be<T>
    where T::ArgsBuilder: InheritEndian {
    fn decode_with<R: Read + ?Sized>(reader: &mut R, args: Args) -> Result<Self, DecodeError> {
        T::decode_with(reader, args).map(Be)
    }
}

impl<Args, T: Encode<Args>> Encode<Args> for Be<T>
    where T::ArgsBuilder: InheritEndian {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, args: Args) -> Result<(), EncodeError> {
        self.0.encode_with(writer, args)
    }
}

impl<T> NamedArgs<dir::Read> for Vec<T> {
    type ArgsBuilder = VecArgsBuilder<Required, T, fn(T) -> T>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<Args, U, T, F> Decode<VecArgs<Args, U, F>> for Vec<T>
    where Args: Iterator, U: Decode<Args::Item>, F: Fn(U) -> T {
    fn decode_with<S: Read + ?Sized>(s: &mut S, args: VecArgs<Args, U, F>) -> Result<Self, DecodeError> {
        args.element_args.map(|arg| U::decode_with(s, arg).map(&args.transform)).collect()
    }
}

impl<T> NamedArgs<dir::Write> for Vec<T> {
    type ArgsBuilder = VecArgsBuilder<Provided<std::iter::Repeat<()>>, T, fn(&T) -> &T>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<Args, U, T, F> Encode<VecArgs<Args, T, F>> for Vec<T>
    where Args: Iterator, U: Encode<Args::Item>, F: Fn(&T) -> &U {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, args: VecArgs<Args, T, F>) -> Result<(), EncodeError> {
        self.deref().encode_with(writer, args)
    }
}

impl<T> NamedArgs<dir::Write> for [T] {
    type ArgsBuilder = VecArgsBuilder<Provided<std::iter::Repeat<()>>, T, fn(&T) -> &T>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<Args, U, T, F> Encode<VecArgs<Args, T, F>> for [T]
    where Args: Iterator, U: Encode<Args::Item>, F: Fn(&T) -> &U {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, args: VecArgs<Args, T, F>) -> Result<(), EncodeError> {
        let mut element_args = args.element_args;
        self.iter().map(&args.transform).try_for_each(|x| {
            const ERR: EncodeError = EncodeError::InvalidArgument("Vec", "not enough arguments");
            let arg = element_args.next().ok_or(ERR)?;
            x.encode_with(writer, arg)
        })
    }
}

impl<T> NamedArgs<dir::Read> for Box<[T]> {
    type ArgsBuilder = VecArgsBuilder<Required, T, fn(T) -> T>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<Args, U, T, F> Decode<VecArgs<Args, U, F>> for Box<[T]>
    where Args: Iterator, U: Decode<Args::Item>, F: Fn(U) -> T {
    fn decode_with<S: Read + ?Sized>(s: &mut S, args: VecArgs<Args, U, F>) -> Result<Self, DecodeError> {
        Vec::<T>::decode_with(s, args).map(Vec::into_boxed_slice)
    }
}
