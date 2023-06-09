//! Interface for encoding and decoding binary data.

use std::io::{Read, Write};
use std::ops::Deref;
use crate::context::{ArgsBuilderFinished, Endian, Context, Provided, Required, NoArgs, VecArgs, VecArgsBuilder, NoEndian, StrArgs, StrArgsBuilder};
use crate::stream::{dir, DecodeError, Direction, EncodeError};

/// Decode binary data to structured in-memory representation.
pub trait Decode<Args = ()>: Context<dir::Read> + Sized {
    /// Decode an instance of `Self` from input stream with the given arguments.
    fn decode_with<R: Read + ?Sized>(reader: &mut R, endian: Self::EndianContext, args: Args) -> Result<Self, DecodeError>;
    /// Decode an instance of `Self` from input stream with default arguments.
    fn decode<R: Read + ?Sized>(reader: &mut R) -> Result<Self, DecodeError>
        where Self::EndianContext: Default, Self::ArgsBuilder: ArgsBuilderFinished<Output = Args> {
        Self::decode_with(reader, Self::EndianContext::default(), Self::args_builder().finish())
    }
}

/// Encode binary data from structured in-memory representation.
pub trait Encode<Args = ()>: Context<dir::Write> {
    /// Encode `self` to the output stream with the given arguments.
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Self::EndianContext, args: Args) -> Result<(), EncodeError>;
    /// Encode `self` to the output stream with default arguments.
    fn encode<W: Write + ?Sized>(&self, writer: &mut W) -> Result<(), EncodeError>
        where Self::EndianContext: Default, Self::ArgsBuilder: ArgsBuilderFinished<Output = Args> {
        self.encode_with(writer, Self::EndianContext::default(), Self::args_builder().finish())
    }
}

impl<'a, T: Context<dir::Write> + ?Sized> Context<dir::Write> for &'a T {
    type EndianContext = T::EndianContext;
    type ArgsBuilder = T::ArgsBuilder;
    fn args_builder() -> Self::ArgsBuilder { T::args_builder() }
}

impl<'a, Args, T: Encode<Args> + ?Sized> Encode<Args> for &'a T {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Self::EndianContext, args: Args) -> Result<(), EncodeError> {
        T::encode_with(self, writer, endian, args)
    }
}

impl<T: Context<dir::Write> + ?Sized> Context<dir::Write> for Box<T> {
    type EndianContext = T::EndianContext;
    type ArgsBuilder = T::ArgsBuilder;
    fn args_builder() -> Self::ArgsBuilder { T::args_builder() }
}

impl<Args, T: Encode<Args> + ?Sized> Encode<Args> for Box<T> {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Self::EndianContext, args: Args) -> Result<(), EncodeError> {
        T::encode_with(self, writer, endian, args)
    }
}

fn encode_iter<W, I, Args>(writer: &mut W, type_name: &'static str,
                           endian: <I::Item as Context<dir::Write>>::EndianContext,
                           iter: I, args: Args) -> Result<(), EncodeError>
    where W: Write + ?Sized, I: IntoIterator, Args: IntoIterator, I::Item: Encode<Args::Item> {
    let mut args = args.into_iter();
    iter.into_iter().try_for_each(|x| {
        let err = EncodeError::InvalidArgument(type_name, "not enough arguments");
        let arg = args.next().ok_or(err)?;
        x.encode_with(writer, endian, arg)
    })
}

impl<Dir: Direction> Context<Dir> for () {
    type EndianContext = NoEndian;
    type ArgsBuilder = NoArgs;
    fn args_builder() -> Self::ArgsBuilder { NoArgs }
}

impl Encode for () {
    fn encode_with<W: Write + ?Sized>(&self, _writer: &mut W, _: NoEndian, _: ()) -> Result<(), EncodeError> { Ok(()) }
}

impl Decode for () {
    fn decode_with<R: Read + ?Sized>(_reader: &mut R, _: NoEndian, _: ()) -> Result<Self, DecodeError> { Ok(()) }
}

/// Marker trait: `U: View<T>` indicates that when we need to encode a value of type `T`, we can
/// encode a value of `U` instead.
pub trait View<T: ?Sized> {}

impl<T> View<T> for T {}
impl<T: ?Sized> View<T> for &'_ T {}
impl<T: ?Sized> View<Box<T>> for &'_ T {}
impl<T> View<Vec<T>> for &'_ [T] {}

/// View into a slice, with every element projected using `P`.
///
/// ```
/// # use bin_data::data::{SliceViewRef, Encode};
/// let data = [(1_i32, "one"), (2_i32, "two")];
/// let strings = SliceViewRef::new(&data, |&(_, s)| s);
/// assert_eq!(strings.into_iter().collect::<Vec<_>>(), vec!["one", "two"]);
/// let mut buffer = Vec::new();
/// strings.encode(&mut buffer).unwrap();
/// assert_eq!(buffer, "onetwo".as_bytes());
/// ```
#[derive(Debug, Copy, Clone)]
pub struct SliceViewRef<'a, T, P> {
    base_slice: &'a [T],
    projector: P,
}

// the `Fn` bound for `P` should help type inference
impl<'a, T, U: ?Sized + 'a, P: Fn(&T) -> &U> SliceViewRef<'a, T, P> {
    /// Create a new view into the slice.
    pub fn new(base_slice: &'a [T], projector: P) -> Self {
        SliceViewRef { base_slice, projector }
    }
}

impl<'a, T, U: 'a, P: Fn(&T) -> &U> View<Vec<U>> for SliceViewRef<'a, T, P> {}
impl<'a, T, U: 'a, P: Fn(&T) -> &U> View<Box<[U]>> for SliceViewRef<'a, T, P> {}

impl<'a, T, U: ?Sized + 'a, P: Fn(&T) -> &U> IntoIterator for SliceViewRef<'a, T, P> {
    type Item = &'a U;
    type IntoIter = std::iter::Map<std::slice::Iter<'a, T>, P>;
    fn into_iter(self) -> Self::IntoIter { self.base_slice.iter().map(self.projector) }
}

impl<'a, 'b, T, U: ?Sized + 'a, P: Fn(&T) -> &U> IntoIterator for &'b SliceViewRef<'a, T, P> {
    type Item = &'b U;
    type IntoIter = std::iter::Map<std::slice::Iter<'b, T>, &'b P>;
    fn into_iter(self) -> Self::IntoIter { self.base_slice.iter().map(&self.projector) }
}

impl<'a, A, B, P> Context<dir::Write> for SliceViewRef<'a, A, P>
    where B: Context<dir::Write> + ?Sized + 'a, P: Fn(&A) -> &B {
    type EndianContext = B::EndianContext;
    type ArgsBuilder = VecArgsBuilder<Provided<std::iter::Repeat<()>>>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<'a, A, B, P, Args> Encode<VecArgs<Args>> for SliceViewRef<'a, A, P>
    where B: ?Sized + 'a, P: Fn(&A) -> &B, Args: Iterator, B: Encode<Args::Item> {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Self::EndianContext, args: VecArgs<Args>) -> Result<(), EncodeError> {
        encode_iter(writer, "SliceViewRef", endian, self, args.element_args)
    }
}

/// View into a slice, with every element projected using `P`.
///
/// ```
/// # use bin_data::context::{Context, Endian, VecArgs, ArgsBuilderFinished};
/// # use bin_data::data::{SliceView, Encode};
/// # use bin_data::stream::dir;
/// let data = [(1_i32, "one"), (2_i32, "two")];
/// let nums = SliceView::new(&data, |&(n, _)| n);
/// assert_eq!(nums.into_iter().collect::<Vec<_>>(), vec![1, 2]);
/// let mut buffer = Vec::new();
/// let args = <Vec<i32> as Context<dir::Write>>::args_builder().finish();
/// nums.encode_with(&mut buffer, Endian::Little, args).unwrap();
/// assert_eq!(buffer, [1, 0, 0, 0, 2, 0, 0, 0]);
/// ```
#[derive(Debug, Copy, Clone)]
pub struct SliceView<'a, T, P> {
    base_slice: &'a [T],
    projector: P,
}

// the `Fn` bound for `P` should help type inference
impl<'a, T, U: 'a, P: Fn(&T) -> U> SliceView<'a, T, P> {
    /// Create a new view into the slice.
    pub fn new(base_slice: &'a [T], projector: P) -> Self {
        SliceView { base_slice, projector }
    }
}

impl<'a, T, U: 'a, P: Fn(&T) -> U> View<Vec<U>> for SliceView<'a, T, P> {}
impl<'a, T, U: 'a, P: Fn(&T) -> U> View<Box<[U]>> for SliceView<'a, T, P> {}

impl<'a, T, U: 'a, P: Fn(&T) -> U> IntoIterator for SliceView<'a, T, P> {
    type Item = U;
    type IntoIter = std::iter::Map<std::slice::Iter<'a, T>, P>;
    fn into_iter(self) -> Self::IntoIter { self.base_slice.iter().map(self.projector) }
}

impl<'a, 'b, T, U: 'a, P: Fn(&T) -> U> IntoIterator for &'b SliceView<'a, T, P> {
    type Item = U;
    type IntoIter = std::iter::Map<std::slice::Iter<'b, T>, &'b P>;
    fn into_iter(self) -> Self::IntoIter { self.base_slice.iter().map(&self.projector) }
}

impl<'a, A, B: Context<dir::Write>, P: Fn(&A) -> B> Context<dir::Write> for SliceView<'a, A, P> {
    type EndianContext = B::EndianContext;
    type ArgsBuilder = VecArgsBuilder<Provided<std::iter::Repeat<()>>>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<'a, A, B, P: Fn(&A) -> B, Args> Encode<VecArgs<Args>> for SliceView<'a, A, P>
    where Args: Iterator, B: Encode<Args::Item> {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Self::EndianContext, args: VecArgs<Args>) -> Result<(), EncodeError> {
        encode_iter(writer, "SliceView", endian, self, args.element_args)
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

            impl<Dir: Direction> Context<Dir> for $t {
                type EndianContext = Endian;
                type ArgsBuilder = NoArgs;
                fn args_builder() -> Self::ArgsBuilder { NoArgs }
            }

            impl Decode for $t {
                fn decode_with<R: Read + ?Sized>(reader: &mut R, endian: Endian, _args: ()) -> Result<Self, DecodeError> {
                    plain_data_decode_with(reader, endian)
                }
            }

            impl Encode for $t {
                fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Endian, _args: ()) -> Result<(), EncodeError> {
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
///
/// Use integers or floating point numbers as [`magic`](crate::stream::Stream::magic)s:
/// ```
/// # use bin_data::data::Le;
/// # use bin_data::stream::Stream;
/// let mut buffer = Vec::new();
/// buffer.magic(Le(42_u16)).unwrap();
/// assert_eq!(buffer, [42, 0]);
/// ```
///
/// Type-level `#[bin_data(endian = "little")]`:
/// ```
/// # use bin_data::data::{Le, Encode};
/// let mut buffer = Vec::new();
/// Le(42_u16).encode(&mut buffer).unwrap();
/// assert_eq!(buffer, [42, 0]);
/// ```
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Le<T>(pub T);

impl<Dir: Direction, T: Context<Dir>> Context<Dir> for Le<T> {
    type EndianContext = NoEndian;
    type ArgsBuilder = T::ArgsBuilder;
    fn args_builder() -> Self::ArgsBuilder { T::args_builder() }
}

impl<Args, T: Decode<Args>> Decode<Args> for Le<T> {
    fn decode_with<R: Read + ?Sized>(reader: &mut R, _: NoEndian, args: Args) -> Result<Self, DecodeError> {
        T::decode_with(reader, Endian::Little.into_context(), args).map(Le)
    }
}

impl<Args, T: Encode<Args>> Encode<Args> for Le<T> {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, _: NoEndian, args: Args) -> Result<(), EncodeError> {
        self.0.encode_with(writer, Endian::Little.into_context(), args)
    }
}

/// Wrapper for big-endian data.
///
/// Use integers or floating point numbers as [`magic`](crate::stream::Stream::magic)s:
/// ```
/// # use bin_data::data::Be;
/// # use bin_data::stream::Stream;
/// let mut buffer = Vec::new();
/// buffer.magic(Be(42_u16)).unwrap();
/// assert_eq!(buffer, [0, 42]);
/// ```
///
/// Type-level `#[bin_data(endian = "little")]`:
/// ```
/// # use bin_data::data::{Be, Encode};
/// let mut buffer = Vec::new();
/// Be(42_u16).encode(&mut buffer).unwrap();
/// assert_eq!(buffer, [0, 42]);
/// ```
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Be<T>(pub T);

impl<Dir: Direction, T: Context<Dir>> Context<Dir> for Be<T> {
    type EndianContext = NoEndian;
    type ArgsBuilder = T::ArgsBuilder;
    fn args_builder() -> Self::ArgsBuilder { T::args_builder() }
}

impl<Args, T: Decode<Args>> Decode<Args> for Be<T> {
    fn decode_with<R: Read + ?Sized>(reader: &mut R, _: NoEndian, args: Args) -> Result<Self, DecodeError> {
        T::decode_with(reader, Endian::Big.into_context(), args).map(Be)
    }
}

impl<Args, T: Encode<Args>> Encode<Args> for Be<T> {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, _: NoEndian, args: Args) -> Result<(), EncodeError> {
        self.0.encode_with(writer, Endian::Big.into_context(), args)
    }
}

impl<T: Context<dir::Read>> Context<dir::Read> for Vec<T> {
    type EndianContext = T::EndianContext;
    type ArgsBuilder = VecArgsBuilder<Required>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::default() }
}

impl<Args, T> Decode<VecArgs<Args>> for Vec<T>
    where Args: Iterator, T: Decode<Args::Item> {
    fn decode_with<S: Read + ?Sized>(s: &mut S, endian: Self::EndianContext, args: VecArgs<Args>) -> Result<Self, DecodeError> {
        args.element_args.map(|arg| T::decode_with(s, endian, arg)).collect()
    }
}

impl<T: Context<dir::Write>> Context<dir::Write> for Vec<T> {
    type EndianContext = T::EndianContext;
    type ArgsBuilder = VecArgsBuilder<Provided<std::iter::Repeat<()>>>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<Args, T> Encode<VecArgs<Args>> for Vec<T>
    where Args: Iterator, T: Encode<Args::Item> {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Self::EndianContext, args: VecArgs<Args>) -> Result<(), EncodeError> {
        self.deref().encode_with(writer, endian, args)
    }
}

impl<T: Context<dir::Write>> Context<dir::Write> for [T] {
    type EndianContext = T::EndianContext;
    type ArgsBuilder = VecArgsBuilder<Provided<std::iter::Repeat<()>>>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::new() }
}

impl<Args, T> Encode<VecArgs<Args>> for [T]
    where Args: Iterator, T: Encode<Args::Item> {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, endian: Self::EndianContext, args: VecArgs<Args>) -> Result<(), EncodeError> {
        encode_iter(writer, "Vec", endian, self, args.element_args)
    }
}

impl<T: Context<dir::Read>> Context<dir::Read> for Box<[T]> {
    type EndianContext = T::EndianContext;
    type ArgsBuilder = VecArgsBuilder<Required>;
    fn args_builder() -> Self::ArgsBuilder { Self::ArgsBuilder::default() }
}

impl<Args, T> Decode<VecArgs<Args>> for Box<[T]>
    where Args: Iterator, T: Decode<Args::Item> {
    fn decode_with<S: Read + ?Sized>(s: &mut S, endian: Self::EndianContext, args: VecArgs<Args>) -> Result<Self, DecodeError> {
        Vec::<T>::decode_with(s, endian, args).map(Vec::into_boxed_slice)
    }
}

impl Context<dir::Read> for String {
    type EndianContext = NoEndian;
    type ArgsBuilder = StrArgsBuilder<Required>;
    fn args_builder() -> StrArgsBuilder<Required> { StrArgsBuilder::default() }
}

impl Decode<StrArgs> for String {
    fn decode_with<R: Read + ?Sized>(reader: &mut R, _: NoEndian, args: StrArgs) -> Result<Self, DecodeError> {
        use DecodeError::IncompleteData;
        let mut buffer = vec![0_u8; args.count];
        reader.read_exact(&mut buffer).map_err(|err| IncompleteData("String", err))?;
        String::from_utf8(buffer).map_err(DecodeError::from)
    }
}

impl Context<dir::Write> for String {
    type EndianContext = NoEndian;
    type ArgsBuilder = NoArgs;
    fn args_builder() -> NoArgs { NoArgs }
}

impl Encode for String {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, _: NoEndian, _: ()) -> Result<(), EncodeError> {
        writer.write_all(self.as_bytes()).map_err(EncodeError::from)
    }
}

impl Context<dir::Read> for Box<str> {
    type EndianContext = NoEndian;
    type ArgsBuilder = StrArgsBuilder<Required>;
    fn args_builder() -> StrArgsBuilder<Required> { StrArgsBuilder::default() }
}

impl Decode<StrArgs> for Box<str> {
    fn decode_with<R: Read + ?Sized>(reader: &mut R, _: NoEndian, args: StrArgs) -> Result<Self, DecodeError> {
        String::decode_with(reader, NoEndian, args).map(String::into_boxed_str)
    }
}

impl View<String> for str {}

impl Context<dir::Write> for str {
    type EndianContext = NoEndian;
    type ArgsBuilder = NoArgs;
    fn args_builder() -> NoArgs { NoArgs }
}

impl Encode for str {
    fn encode_with<W: Write + ?Sized>(&self, writer: &mut W, _: NoEndian, _: ()) -> Result<(), EncodeError> {
        writer.write_all(self.as_bytes()).map_err(EncodeError::from)
    }
}
