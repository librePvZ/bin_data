//! Supporting type-level construct for named arguments.

use std::marker::PhantomData;
use crate::stream::Direction;

/// Specify the named arguments used for decoding `Self`.
pub trait NamedArgs<Dir: Direction> {
    /// The argument builder type.
    type ArgsBuilder;
    /// Create an argument builder with default settings.
    fn args_builder() -> Self::ArgsBuilder;
}

/// Indicates that all arguments is supplied.
pub trait ArgsBuilderFinished {
    /// The arguments type to be built by this argument builder.
    type Output;
    /// Finish building and produce the arguments.
    fn finish(self) -> Self::Output;
}

/// Trivial arguments builder for `()`.
#[derive(Default, Debug, Copy, Clone)]
pub struct NoArgs;

impl ArgsBuilderFinished for NoArgs {
    type Output = ();
    fn finish(self) {}
}

/// Placeholder for a required field without a default value.
#[derive(Default, Debug, Copy, Clone)]
pub struct Required;

/// A provided field. This is deliberately not [`Default`], to prevent accidentally supplying
/// default values for required arguments.
#[derive(Debug, Copy, Clone)]
pub struct Provided<T>(pub T);

/// Arguments for encoding or decoding a [`Vec`].
#[derive(Debug, Copy, Clone)]
pub struct VecArgs<Args, U, F> {
    pub(crate) element_args: Args,
    pub(crate) transform: F,
    pub(crate) _marker: PhantomData<fn(U)>,
}

/// Named arguments builder for [`VecArgs`].
#[derive(Debug, Copy, Clone)]
pub struct VecArgsBuilder<Args, U, F> {
    element_args: Args,
    transform: F,
    _marker: PhantomData<fn(U)>,
}

impl<T> VecArgsBuilder<Required, T, fn(T) -> T> {
    pub(crate) fn new() -> Self {
        VecArgsBuilder {
            element_args: Required,
            transform: std::convert::identity,
            _marker: PhantomData,
        }
    }
}

impl<T> VecArgsBuilder<std::iter::Repeat<()>, T, fn(&T) -> &T> {
    pub(crate) fn new() -> Self {
        VecArgsBuilder {
            element_args: std::iter::repeat(()),
            transform: |x| x,
            _marker: PhantomData,
        }
    }
}

impl<U, F> VecArgsBuilder<Required, U, F> {
    /// Specify a series of arguments for decoding the elements in the [`Vec`].
    pub fn args<I: IntoIterator>(self, args: I) -> VecArgsBuilder<Provided<I::IntoIter>, U, F> {
        VecArgsBuilder {
            element_args: Provided(args.into_iter()),
            transform: self.transform,
            _marker: self._marker,
        }
    }

    /// Specify the expected number of elements in the [`Vec`].
    pub fn count(self, n: usize) -> VecArgsBuilder<Provided<impl Iterator<Item = ()>>, U, F> {
        VecArgsBuilder {
            element_args: Provided(std::iter::repeat(()).take(n)),
            transform: self.transform,
            _marker: self._marker,
        }
    }
}

impl<Args, U, F> VecArgsBuilder<Provided<Args>, U, F> {
    /// Specify a shared argument for decoding all the elements in the [`Vec`].
    pub fn arg<A>(self, arg: A) -> VecArgsBuilder<Provided<impl Iterator<Item = A>>, U, F>
        where A: Clone + 'static, Args: Iterator<Item = ()> {
        VecArgsBuilder {
            element_args: Provided(self.element_args.0.map(move |()| arg.clone())),
            transform: self.transform,
            _marker: self._marker,
        }
    }

    /// Transform the arguments before using it to decode the elements in the [`Vec`].
    pub fn map_arg<B, G>(self, f: G) -> VecArgsBuilder<Provided<impl Iterator<Item = B>>, U, F>
        where Args: Iterator, G: FnMut(Args::Item) -> B {
        VecArgsBuilder {
            element_args: Provided(self.element_args.0.map(f)),
            transform: self.transform,
            _marker: self._marker,
        }
    }
}

impl<Args, U, F> VecArgsBuilder<Args, U, F> {
    /// Specify a function for transforming the result of decoding.
    pub fn map<V, T, G: Fn(V) -> T>(self, f: G) -> VecArgsBuilder<Args, V, G> {
        VecArgsBuilder {
            element_args: self.element_args,
            transform: f,
            _marker: PhantomData,
        }
    }
}

impl<Args, U, F> ArgsBuilderFinished for VecArgsBuilder<Provided<Args>, U, F> {
    type Output = VecArgs<Args, U, F>;
    fn finish(self) -> Self::Output {
        VecArgs {
            element_args: self.element_args.0,
            transform: self.transform,
            _marker: self._marker,
        }
    }
}

/// Endianness for integers, floating-point numbers, etc.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Endian {
    /// Little-endian.
    Little,
    /// Big-endian.
    Big,
}

/// Named argument builder for data with endianness.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct EndianBuilder<E> {
    endian: E,
}

impl EndianBuilder<Required> {
    /// Set the `endian` parameter for this named argument builder.
    pub fn endian(self, endian: Endian) -> EndianBuilder<Provided<Endian>> {
        EndianBuilder { endian: Provided(endian) }
    }
}

impl ArgsBuilderFinished for EndianBuilder<Provided<Endian>> {
    type Output = Endian;
    fn finish(self) -> Endian { self.endian.0 }
}

/// Common interface for a named argument builder to inherit the [`Endian`] parameter.
pub trait InheritEndian {
    /// Result builder type after inheriting the [`Endian`].
    type WithEndian;
    /// Try to inherit the [`Endian`] as the `endian` parameter for this named argument builder. If
    /// the `endian` parameter is already explicitly set, ignore this request.
    fn inherit_endian(self, endian: Endian) -> Self::WithEndian;
}

impl InheritEndian for Required {
    type WithEndian = Provided<Endian>;
    fn inherit_endian(self, endian: Endian) -> Self::WithEndian { Provided(endian) }
}

impl InheritEndian for Provided<Endian> {
    type WithEndian = Self;
    fn inherit_endian(self, _endian: Endian) -> Self::WithEndian { self }
}

impl InheritEndian for NoArgs {
    type WithEndian = Self;
    fn inherit_endian(self, _endian: Endian) -> Self::WithEndian { self }
}

impl<Args, U, F> InheritEndian for VecArgsBuilder<Args, U, F> {
    type WithEndian = Self;
    fn inherit_endian(self, _endian: Endian) -> Self::WithEndian { self }
}

impl<E: InheritEndian> InheritEndian for EndianBuilder<E> {
    type WithEndian = EndianBuilder<E::WithEndian>;
    fn inherit_endian(self, endian: Endian) -> Self::WithEndian {
        EndianBuilder { endian: self.endian.inherit_endian(endian) }
    }
}
