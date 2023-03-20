use proc_macro2::{TokenStream, Ident};
use quote::{quote, quote_spanned};
use crate::input::{ArgFieldAssign, Direction, Entry, Field, FieldKind, Input, KnownAttribute};

pub fn extract_struct(input: &Input, result: &mut TokenStream) {
    let Input {
        known_attrs: _,
        attrs,
        vis,
        struct_token,
        name,
        generics,
        brace_token,
        entries,
    } = input;
    result.extend(quote! { #(#attrs)* #vis #struct_token #name #generics });
    brace_token.surround(result, |tokens| {
        let fields = entries.iter().filter_map(|entry| match entry {
            Entry::Field(field @ Field { kind: FieldKind::Field(_), .. }) => Some(field),
            _ => None,
        });
        tokens.extend(quote! { #(#fields),* })
    });
}

pub fn impl_decode(input: &Input, result: &mut TokenStream) {
    let Input {
        known_attrs,
        name,
        generics,
        entries,
        ..
    } = input;
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let mut global_endian = TokenStream::new();
    for attr in known_attrs {
        match attr {
            KnownAttribute::ArgsAssign { .. } => unreachable!(),
            KnownAttribute::ArgsDecl { direction: Direction::Encode, .. } => {}
            KnownAttribute::Endian { endian_token, value } => global_endian.extend({
                let inherit_endian = Ident::new("inherit_endian", endian_token.span());
                quote!(.#inherit_endian(#value))
            }),
            KnownAttribute::ArgsDecl { direction: Direction::Both | Direction::Decode, .. } => {
                todo!()
            }
            KnownAttribute::Encode(_) | KnownAttribute::Decode(_) => {}
        }
    }
    let fields = entries.iter().filter_map(|entry| match entry {
        Entry::Field(Field { name, kind: FieldKind::Field(_), .. }) => Some(name),
        _ => None,
    });
    let entries = entries.iter().map(|entry| match entry {
        Entry::Directive(directive) => quote!(reader.#directive?;),
        Entry::Field(Field { name, r#type, known_attrs, .. }) => {
            let mut args = TokenStream::new();
            let mut local_endian = TokenStream::new();
            let mut decode = None;
            for attr in known_attrs {
                match attr {
                    KnownAttribute::ArgsDecl { .. } => unreachable!(),
                    KnownAttribute::Encode(_) | KnownAttribute::ArgsAssign { direction: Direction::Encode, .. } => {}
                    KnownAttribute::Decode(val) => decode = Some(val),
                    KnownAttribute::Endian { endian_token, value } => local_endian.extend({
                        let inherit_endian = Ident::new("inherit_endian", endian_token.span());
                        quote!(.#inherit_endian(#value))
                    }),
                    KnownAttribute::ArgsAssign { direction: Direction::Both | Direction::Decode, fields, .. } => {
                        for ArgFieldAssign { name, value, .. } in fields {
                            args.extend(quote!(.#name(#value)));
                        }
                    }
                }
            }
            match decode {
                Some(decode) => quote!(let #name: #r#type = #decode;),
                None => quote_spanned! { name.span() =>
                    let #name: #r#type = {
                        let args = <#r#type as NamedArgs<dir::Read>>::args_builder();
                        <#r#type>::decode_with(reader, ArgsBuilderFinished::finish(
                            args #args #local_endian #global_endian
                        ))?
                    };
                },
            }
        }
    });
    result.extend(quote! {
        impl #impl_generics ::bin_data::named_args::NamedArgs<::bin_data::stream::dir::Read>
            for #name #type_generics #where_clause {
            type ArgsBuilder = ::bin_data::named_args::NoArgs;
            fn args_builder() -> Self::ArgsBuilder { ::bin_data::named_args::NoArgs }
        }
        impl #impl_generics ::bin_data::data::Decode for #name #type_generics #where_clause {
            #[allow(unused_import)]
            fn decode_with<R: std::io::Read + ?Sized>(reader: &mut R, args: ())
                -> Result<Self, ::bin_data::stream::DecodeError> {
                use ::bin_data::stream::{Stream, dir};
                use ::bin_data::named_args::{NamedArgs, InheritEndian, ArgsBuilderFinished};
                #(#entries)*
                Ok(Self { #(#fields),* })
            }
        }
    });
}
