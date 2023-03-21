//! Supporting type-level construct for named arguments.

use crate::stream::Direction;

/// Endianness for integers, floating-point numbers, etc.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Endian {
    /// Little-endian.
    Little,
    /// Big-endian.
    Big,
}

/// Indicate the endianness is not determined at runtime.
#[derive(Default, Debug, Copy, Clone)]
pub struct NoEndian;

mod sealed {
    pub trait EndianContext: Copy + From<super::Endian> {}
    impl EndianContext for super::Endian {}
    impl EndianContext for super::NoEndian {}
    impl From<super::Endian> for super::NoEndian {
        fn from(_: super::Endian) -> Self { super::NoEndian }
    }
}

/// Specify the named arguments used for decoding `Self`.
pub trait Context<Dir: Direction> {
    /// Context containing the desired endianness.
    type EndianContext: sealed::EndianContext;
    /// The argument builder type.
    type ArgsBuilder;
    /// Create an argument builder with default settings.
    fn args_builder() -> Self::ArgsBuilder;
    /// Create an argument builder with default settings.
    fn args_builder_of_val(&self) -> Self::ArgsBuilder { Self::args_builder() }
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
pub struct VecArgs<Args> {
    pub(crate) element_args: Args,
}

/// Named arguments builder for [`VecArgs`].
#[derive(Default, Debug, Copy, Clone)]
pub struct VecArgsBuilder<Args> {
    element_args: Args,
}

impl VecArgsBuilder<Provided<std::iter::Repeat<()>>> {
    pub(crate) fn new() -> Self {
        VecArgsBuilder { element_args: Provided(std::iter::repeat(())) }
    }
}

impl<Args> VecArgsBuilder<Args> {
    /// Specify a series of arguments for decoding the elements in the [`Vec`].
    pub fn args<I: IntoIterator>(self, args: I) -> VecArgsBuilder<Provided<I::IntoIter>> {
        VecArgsBuilder { element_args: Provided(args.into_iter()) }
    }
}

impl VecArgsBuilder<Required> {
    /// Specify the expected number of elements in the [`Vec`].
    pub fn count(self, n: usize) -> VecArgsBuilder<Provided<impl Iterator<Item = ()>>> {
        VecArgsBuilder { element_args: Provided(std::iter::repeat(()).take(n)) }
    }
}

impl<Args> VecArgsBuilder<Provided<Args>> {
    /// Specify a shared argument for decoding all the elements in the [`Vec`].
    pub fn arg<A>(self, arg: A) -> VecArgsBuilder<Provided<impl Iterator<Item = A>>>
        where A: Clone + 'static, Args: Iterator<Item = ()> {
        VecArgsBuilder { element_args: Provided(self.element_args.0.map(move |()| arg.clone())) }
    }

    /// Transform the arguments before using it to decode the elements in the [`Vec`].
    pub fn map_arg<B, G>(self, f: G) -> VecArgsBuilder<Provided<impl Iterator<Item = B>>>
        where Args: Iterator, G: FnMut(Args::Item) -> B {
        VecArgsBuilder { element_args: Provided(self.element_args.0.map(f)) }
    }
}

impl<Args> ArgsBuilderFinished for VecArgsBuilder<Provided<Args>> {
    type Output = VecArgs<Args>;
    fn finish(self) -> Self::Output {
        VecArgs { element_args: self.element_args.0 }
    }
}
