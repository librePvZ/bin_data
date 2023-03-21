//! Context for encoding and decoding.
//!
//! The types can be roughly grouped into two categories:
//! - [`Context::EndianContext`]: explicit endianness specification and inheritance.
//!     - [`Endian`]: little-endian or big-endian.
//!     - [`NoEndian`]: endianness is not decided at runtime.
//! - [`Context::ArgsBuilder`]: type-level construct for named arguments.
//!     - [`NoArgs`]: no argument at all, or `Args = ()`.
//!     - [`VecArgs`] and [`VecArgsBuilder`]: arguments for [`Vec`], [`slice`]s, etc.
//!     - [`StrArgs`] and [`StrArgsBuilder`]: arguments for [`String`], [`str`], etc.
//!
//! Types in this module might appear in error messages, here is an overview:
//! - **expected enum [`Endian`], found struct [`NoEndian`]**: endianness for one of the fields
//!     must be specified. Add the following attribute: `#[bin_data(endian = "...")]`, either to
//!     the whole `struct`, or to individual fields.
//! - **expected struct [`NoEndian`], found enum [`Endian`]**: opposite situation to the previous
//!     one, one of the fields require no runtime-specified endianness, but endianness is specified
//!     explicitly using `#[bin_data(endian = "...")]`. Remove that superfluous attribute.
//! - **_some argument builder_ does not implement [`ArgsBuilderFinished`]**: some [`Required`]
//!     argument is not specified. Specify it in `#[bin_data(args { ... })]`. The arguments should
//!     be given as `name = value`, separated and optionally ended by commas. Under the hood, it
//!     calls the method `name` with argument `value` on the argument builder. For instance, the
//!     following code set the expected length of a [`Vec`] by calling [`VecArgsBuilder::count`]:
//!     ```
//!     # bin_data_macros::bin_data! {
//!     #     #[bin_data(endian = "inherit")]
//!     #     struct Test {
//!     #[bin_data(args:decode { count = 42 })]
//!     xs: Vec<u8>,
//!     #     }
//!     # }
//!     ```
//! See also each type's documentation for detailed explanation.

use crate::stream::Direction;

/// Endianness for integers, floating-point numbers, etc.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Endian {
    /// Little-endian.
    Little,
    /// Big-endian.
    Big,
}

impl Endian {
    /// Convert into an [`EndianContext`](Context::EndianContext).
    ///
    /// We deliberately do not use [`From`] and [`Into`] to avoid the compiler suggesting adding
    /// `.into()`, because the error should not be fixed that way.
    pub fn into_context<C: sealed::EndianContext>(self) -> C { C::with_endian(self) }
}

/// Indicate the endianness is not determined at runtime.
#[derive(Default, Debug, Copy, Clone)]
pub struct NoEndian;

mod sealed {
    use super::{Endian, NoEndian};
    pub trait EndianContext: Copy {
        fn with_endian(endian: Endian) -> Self;
    }
    impl EndianContext for Endian { fn with_endian(endian: Endian) -> Self { endian } }
    impl EndianContext for NoEndian { fn with_endian(_endian: Endian) -> Self { NoEndian } }
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
///
/// The `#[bin_data(args { ... })]` attribute specifies named arguments for encoding or decoding
/// that field. After all required arguments are specified, the argument builder is transformed
/// into a type implementing this trait, and we call [`ArgsBuilderFinished::finish`] to get the
/// final arguments. If the compiler reports missing implementation for this trait, then there are
/// some [`Required`] arguments not [`Provided`].
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
    /// Iterator of the actual arguments.
    pub element_args: Args,
}

/// Named arguments builder for [`VecArgs`].
///
/// For maximum flexibility, we specify arguments for each element in the [`Vec`]. If the elements
/// require no arguments (`Args = ()`), we only need a length for decoding, and we use [`count`]
/// for this purpose. If the same argument could be shared by every element in the [`Vec`], we can
/// use [`arg`] to specify that argument (requires [`Clone`]). Finally, for full control, we can
/// use [`args`] to specify a separate argument for each element. Besides, [`map_arg`] could be
/// used to transform the arguments.
/// ```
/// # use bin_data::context::{VecArgsBuilder, ArgsBuilderFinished, Required, VecArgs};
/// fn builder() -> VecArgsBuilder<Required> { VecArgsBuilder::default() }
/// fn get_args<I: Iterator>(args: VecArgs<I>) -> Vec<I::Item> { args.element_args.collect() }
/// assert_eq!(vec![(), (), ()], get_args(builder().count(3).finish()));
/// assert_eq!(vec![42, 42, 42], get_args(builder().count(3).arg(42).finish()));
/// assert_eq!(vec![1, 2, 3], get_args(builder().args([1, 2, 3]).finish()));
/// assert_eq!(vec![5, 6, 7], get_args(builder().args([1, 2, 3]).map_arg(|x| x + 4).finish()));
/// ```
///
/// # Note
/// The argument [`arg`] must be specified after [`count`].
///
/// [`count`]: VecArgsBuilder::count
/// [`arg`]: VecArgsBuilder::arg
/// [`args`]: VecArgsBuilder::args
/// [`map_arg`]: VecArgsBuilder::map_arg
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

/// Arguments for encoding or decoding a [`str`], [`String`], etc.
#[derive(Debug, Copy, Clone)]
pub struct StrArgs {
    /// Number of bytes in this string.
    pub count: usize,
}

/// Named arguments builder for [`StrArgs`].
///
/// This builder is relatively simple, use [`count`] to specify the length of the string.
/// ```
/// # use bin_data::context::{Required, StrArgs, StrArgsBuilder, ArgsBuilderFinished};
/// assert_eq!(StrArgsBuilder::<Required>::default().count(42).finish().count, 42);
/// ```
///
/// [`count`]: StrArgsBuilder::count
#[derive(Default, Debug, Copy, Clone)]
pub struct StrArgsBuilder<N> {
    count: N,
}

impl StrArgsBuilder<Required> {
    /// Specify the expected number of bytes in the string.
    pub fn count(self, n: usize) -> StrArgsBuilder<Provided<usize>> {
        StrArgsBuilder { count: Provided(n) }
    }
}

impl ArgsBuilderFinished for StrArgsBuilder<Provided<usize>> {
    type Output = StrArgs;
    fn finish(self) -> StrArgs { StrArgs { count: self.count.0 } }
}
