use proc_macro2::TokenStream;
use quote::quote;
use crate::input::{Entry, Field, FieldKind, Input};

pub fn extract_struct(input: &Input) -> TokenStream {
    let Input {
        attrs,
        vis,
        struct_token,
        name,
        generics,
        brace_token,
        entries,
    } = input;
    let mut result = TokenStream::new();
    result.extend(quote! { #(#attrs)* #vis #struct_token #name #generics });
    brace_token.surround(&mut result, |tokens| {
        let fields = entries.iter().filter_map(|entry| match entry {
            Entry::Field(field @ Field { kind: FieldKind::Field(_), .. }) => Some(field),
            _ => None,
        });
        tokens.extend(quote! { #(#fields),* })
    });
    result
}
