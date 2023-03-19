use itertools::Itertools;
use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::{Token, parenthesized, braced, Attribute, Visibility, Type, Generics, MacroDelimiter, Meta};
use syn::parse::{Parse, ParseStream};
use syn::token::{Brace, Paren};

pub struct Input {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub struct_token: Token![struct],
    pub name: Ident,
    pub generics: Generics,
    pub brace_token: Brace,
    pub entries: Punctuated<Entry, Token![,]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let contents;
        Ok(Input {
            attrs: Attribute::parse_outer(input)?,
            vis: input.parse()?,
            struct_token: input.parse()?,
            name: input.parse()?,
            generics: input.parse()?,
            brace_token: braced!(contents in input),
            entries: contents.parse_terminated(Entry::parse, Token![,])?,
        })
    }
}

pub enum Entry {
    Directive(TokenStream),
    Field(Field),
}

impl Parse for Entry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![@]) {
            input.parse().map(Entry::Directive)
        } else {
            input.parse().map(Entry::Field)
        }
    }
}

pub struct Directive {
    pub at_token: Token![@],
    pub directive: Ident,
    pub paren_token: Paren,
    pub arguments: TokenStream,
}

impl Parse for Directive {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let arguments;
        Ok(Directive {
            at_token: input.parse()?,
            directive: input.parse()?,
            paren_token: parenthesized!(arguments in input),
            arguments: arguments.parse()?,
        })
    }
}

impl ToTokens for Directive {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.directive.to_tokens(tokens);
        self.paren_token.surround(tokens, |tokens| {
            self.arguments.to_tokens(tokens);
        });
    }
}

pub struct Field {
    pub known_attrs: Vec<KnownAttribute>,
    pub attrs: Vec<Attribute>,
    pub kind: FieldKind,
    pub name: Ident,
    pub colon_token: Token![:],
    pub r#type: Type,
}

impl Parse for Field {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let (known_attrs, attrs) = attrs.into_iter()
            .map(KnownAttribute::try_from)
            .partition_result();
        Ok(Field {
            known_attrs,
            attrs,
            kind: input.parse()?,
            name: input.parse()?,
            colon_token: input.parse()?,
            r#type: input.parse()?,
        })
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.attrs.iter().for_each(|attr| attr.to_tokens(tokens));
        self.kind.to_tokens(tokens);
        self.name.to_tokens(tokens);
        self.colon_token.to_tokens(tokens);
        self.r#type.to_tokens(tokens);
    }
}

pub struct KnownAttribute {
    pub delimiter: MacroDelimiter,
    pub tokens: TokenStream,
}

impl TryFrom<Attribute> for KnownAttribute {
    type Error = Attribute;
    fn try_from(attr: Attribute) -> Result<KnownAttribute, Attribute> {
        if !attr.path().is_ident("bin_data") { return Err(attr); }
        let Meta::List(list) = attr.meta else { return Err(attr); };
        Ok(KnownAttribute {
            delimiter: list.delimiter,
            tokens: list.tokens,
        })
    }
}

pub enum FieldKind {
    Field(Visibility),
    Temp(Token![let]),
}

impl Parse for FieldKind {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![let]) {
            input.parse().map(FieldKind::Temp)
        } else {
            input.parse().map(FieldKind::Field)
        }
    }
}

impl ToTokens for FieldKind {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            FieldKind::Field(vis) => vis.to_tokens(tokens),
            FieldKind::Temp(let_token) => let_token.to_tokens(tokens),
        }
    }
}
