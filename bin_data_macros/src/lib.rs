//! Supporting macros for `bin_data`.

#![warn(missing_docs)]

mod input;
mod code_gen;

use proc_macro2::TokenStream;
use syn::parse_macro_input;
use crate::code_gen::{extract_args, extract_struct, impl_decode};
use crate::input::{Entry, Input};

/// Declare a binary data format.
#[proc_macro]
pub fn bin_data(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as Input);
    let mut result = TokenStream::new();
    extract_struct(&input, &mut result);
    let args = extract_args(&input.known_attrs);
    let field_args = input.entries.iter()
        .map(|entry| match entry {
            Entry::Directive(_) => None,
            Entry::Field(field) => Some(extract_args(&field.known_attrs)),
        })
        .collect::<Vec<_>>();
    impl_decode(&input, &args, &field_args, &mut result);
    result.into()
}
