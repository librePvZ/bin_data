//! Supporting macros for `bin_data`.

#![warn(missing_docs)]

mod input;
mod code_gen;

use proc_macro2::TokenStream;
use syn::parse_macro_input;
use crate::code_gen::{extract_struct, impl_decode};
use crate::input::Input;

/// Declare a binary data format.
#[proc_macro]
pub fn bin_data(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as Input);
    let mut result = TokenStream::new();
    extract_struct(&input, &mut result);
    impl_decode(&input, &mut result);
    result.into()
}
