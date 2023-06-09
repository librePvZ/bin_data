use itertools::Itertools;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Expr, LitStr, spanned::Spanned};
use crate::input::{ArgFieldAssign, ArgFieldDecl, EndianConfig, Entry, Field, Input, KnownAttribute, WithToken};

pub fn extract_struct(input: &Input, result: &mut TokenStream) {
    let Input {
        known_attrs: _,
        attrs,
        vis,
        struct_token,
        name,
        generics,
        brace_token,
        entries: _,
    } = input;
    result.extend(quote! { #(#attrs)* #vis #struct_token #name #generics });
    brace_token.surround(result, |tokens| {
        let fields = input.fields();
        tokens.extend(quote! { #(#fields),* })
    });
}

#[derive(Default)]
pub struct ExtractedArgs<'a> {
    endian: Option<&'a WithToken<LitStr, EndianConfig>>,
    encode: Config<'a>,
    decode: Config<'a>,
    errors: TokenStream,
}

#[derive(Default)]
pub struct Config<'a> {
    args_decl: Vec<&'a ArgFieldDecl>,
    args_assign: Vec<&'a ArgFieldAssign>,
    calculate: Option<&'a Expr>,
}

impl Config<'_> {
    pub fn arg_setters(&self) -> TokenStream {
        assert!(self.args_decl.is_empty());
        self.args_assign.iter().copied()
            .map(|ArgFieldAssign { name, value, .. }| quote!(.#name(#value)))
            .collect::<TokenStream>()
    }
}

pub fn extract_args(known_attrs: &[KnownAttribute]) -> ExtractedArgs {
    let mut args = ExtractedArgs::default();
    for attr in known_attrs {
        macro_rules! set {
            ($errors:expr, $tag:literal, $field:expr, $value:expr) => {
                if $field.is_none() {
                    $field = Some($value);
                } else {
                    let msg = concat!("duplicated option `", $tag, "`");
                    $errors.extend(quote_spanned!($value.span() => compile_error!(#msg);));
                }
            }
        }
        match attr {
            KnownAttribute::Endian(endian) => set!(args.errors, "endian", args.endian, endian),
            KnownAttribute::Encode(value) => set!(args.errors, "encode", args.encode.calculate, value),
            KnownAttribute::Decode(value) => set!(args.errors, "decode", args.decode.calculate, value),
            KnownAttribute::ArgsAssign { direction, fields, .. } => direction.dispatch(
                &mut args.encode.args_assign,
                &mut args.decode.args_assign,
                |target| target.extend(fields.iter()),
            ),
            KnownAttribute::ArgsDecl { direction, fields, .. } => direction.dispatch(
                &mut args.encode.args_decl,
                &mut args.decode.args_decl,
                |target| target.extend(fields.iter()),
            ),
        }
    }
    args
}

fn decide_endian(
    field_span: Span,
    local_endian: Option<&WithToken<LitStr, EndianConfig>>,
    global_endian: EndianConfig,
) -> TokenStream {
    match local_endian {
        None if global_endian != EndianConfig::None => quote_spanned!(field_span => endian.into_context()),
        None => quote_spanned!(field_span => ::bin_data::context::NoEndian),
        Some(local_endian) => match local_endian.value {
            EndianConfig::None => quote_spanned!(local_endian.span() => ::bin_data::context::NoEndian),
            EndianConfig::Little => quote_spanned!(local_endian.span() => ::bin_data::context::Endian::Little),
            EndianConfig::Big => quote_spanned!(local_endian.span() => ::bin_data::context::Endian::Big),
            EndianConfig::Inherit => quote_spanned!(local_endian.span() => endian),
        },
    }
}

fn decode_entry(
    global_endian: EndianConfig,
    entry: &Entry,
    args: &Option<ExtractedArgs>,
) -> TokenStream {
    match entry {
        Entry::Directive(directive) => quote!(reader.#directive?;),
        Entry::Field(Field { name, r#type, .. }) => {
            let args = args.as_ref().unwrap();
            let arg_setters = args.decode.arg_setters();
            let endian = decide_endian(name.span(), args.endian, global_endian);
            match args.decode.calculate {
                Some(decode) => quote!(let #name: #r#type = #decode;),
                None => quote_spanned! { name.span() =>
                    let #name: #r#type = <#r#type>::decode_with(
                        reader, #endian,
                        ArgsBuilderFinished::finish(
                            <#r#type as Context<dir::Read>>::args_builder() #arg_setters
                        ),
                    )?;
                },
            }
        }
    }
}

pub fn impl_decode(
    input: &Input,
    args: &ExtractedArgs,
    field_args: &[Option<ExtractedArgs>],
    result: &mut TokenStream,
) {
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let fields = input.fields().map(|field| &field.name);
    let global_endian = args.endian.map_or(EndianConfig::None, |t| t.value);
    let entries = input.entries.iter().zip_eq(field_args)
        .map(|(entry, arg)| decode_entry(global_endian, entry, arg));
    let endian_overwrite = global_endian.endian_overwrite();
    let global_endian = global_endian.endian_input();
    let name = &input.name;
    result.extend(quote! {
        impl #impl_generics ::bin_data::context::Context<::bin_data::stream::dir::Read>
            for #name #type_generics #where_clause {
            type EndianContext = #global_endian;
            type ArgsBuilder = ::bin_data::context::NoArgs;
            fn args_builder() -> Self::ArgsBuilder { ::bin_data::context::NoArgs }
        }
        impl #impl_generics ::bin_data::data::Decode for #name #type_generics #where_clause {
            #[allow(unused_import)]
            fn decode_with<R: std::io::Read + ?Sized>(reader: &mut R, endian: #global_endian, args: ())
                -> Result<Self, ::bin_data::stream::DecodeError> {
                #endian_overwrite
                use ::bin_data::stream::{Stream, dir};
                use ::bin_data::context::{Context, ArgsBuilderFinished};
                #(#entries)*
                Ok(Self { #(#fields),* })
            }
        }
    });
}

fn encode_entry(
    global_endian: EndianConfig,
    entry: &Entry,
    args: &Option<ExtractedArgs>,
) -> TokenStream {
    match entry {
        Entry::Directive(directive) => quote!(writer.#directive?;),
        Entry::Field(Field { name, r#type, .. }) => {
            let args = args.as_ref().unwrap();
            let arg_setters = args.encode.arg_setters();
            let endian = decide_endian(name.span(), args.endian, global_endian);
            let builder = if args.encode.calculate.is_none() {
                quote_spanned!(name.span() => <#r#type as Context<dir::Write>>::args_builder())
            } else {
                quote_spanned!(name.span() => Context::<dir::Write>::args_builder_of_val(&#name))
            };
            quote_spanned! { name.span() =>
                #name.encode_with(writer, #endian, ArgsBuilderFinished::finish(#builder #arg_setters))?;
            }
        }
    }
}

pub fn impl_encode(
    input: &Input,
    args: &ExtractedArgs,
    field_args: &[Option<ExtractedArgs>],
    result: &mut TokenStream,
) {
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let fields = input.fields().map(|field| &field.name);
    let entries = input.entries.iter().zip_eq(field_args);
    let temps = entries.clone()
        .filter_map(|(entry, arg)| {
            let field = entry.as_temp()?;
            Some((&field.name, &field.r#type, arg.as_ref().unwrap()))
        })
        .map(|(name, r#type, arg)| match arg.encode.calculate {
            Some(value) => quote! {
                let #name = ::bin_data::data::assert_is_view::<#r#type, _>(#value);
            },
            None => quote_spanned! { name.span() =>
                let #name: #r#type = compile_error!("temporary field requires an `encode` attribute");
            },
        });
    let global_endian = args.endian.map_or(EndianConfig::None, |t| t.value);
    let entries = entries.clone()
        .filter(|&(_, arg)| match arg.as_ref() {
            None => true,
            Some(arg) => arg.decode.calculate.is_none(),
        })
        .map(|(entry, arg)| encode_entry(global_endian, entry, arg));
    let endian_overwrite = global_endian.endian_overwrite();
    let global_endian = global_endian.endian_input();
    let name = &input.name;
    result.extend(quote! {
        impl #impl_generics ::bin_data::context::Context<::bin_data::stream::dir::Write>
            for #name #type_generics #where_clause {
            type EndianContext = #global_endian;
            type ArgsBuilder = ::bin_data::context::NoArgs;
            fn args_builder() -> Self::ArgsBuilder { ::bin_data::context::NoArgs }
        }
        impl #impl_generics ::bin_data::data::Encode for #name #type_generics #where_clause {
            #[allow(unused_import)]
            fn encode_with<W: std::io::Write + ?Sized>(&self, writer: &mut W, endian: #global_endian, args: ())
                -> Result<(), ::bin_data::stream::EncodeError> {
                #endian_overwrite
                use ::bin_data::stream::{Stream, dir};
                use ::bin_data::context::{Context, ArgsBuilderFinished};
                #[allow(unused_variables)]
                let Self { #(#fields),* } = self;
                #(#temps)*
                #(#entries)*
                Ok(())
            }
        }
    });
}
