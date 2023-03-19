//! Supporting macros for `bin_data`.

#![warn(missing_docs)]

mod input;
mod code_gen;

use syn::parse_macro_input;
use crate::code_gen::extract_struct;
use crate::input::Input;

/// Declare a binary data format.
#[proc_macro]
pub fn bin_data(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as Input);
    let struct_def = extract_struct(&input);
    struct_def.into()
}
