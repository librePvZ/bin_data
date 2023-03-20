use itertools::Itertools;
use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::{Token, parenthesized, braced, Attribute, Visibility, Type, Generics, Meta, Expr, Error};
use syn::parse::{Parse, ParseStream};
use syn::token::{Brace, Paren};

/// Input for the macro. Looks like a `struct` definition.
pub struct Input {
    pub known_attrs: Vec<KnownAttribute>,
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub struct_token: Token![struct],
    pub name: Ident,
    pub generics: Generics,
    pub brace_token: Brace,
    pub entries: Punctuated<Entry, Token![,]>,
}

impl Input {
    pub fn fields(&self) -> impl Iterator<Item = &Field> {
        self.entries.iter().filter_map(Entry::as_field)
    }
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let (ResultVec(known_attrs), attrs) = attrs.into_iter()
            .map(|attr| KnownAttribute::new(attr, false))
            .partition_result();
        let contents;
        Ok(Input {
            known_attrs: known_attrs?,
            attrs,
            vis: input.parse()?,
            struct_token: input.parse()?,
            name: input.parse()?,
            generics: input.parse()?,
            brace_token: braced!(contents in input),
            entries: Punctuated::parse_terminated(&contents)?,
        })
    }
}

pub enum Entry {
    /// Stream directives: `@directive(arguments...)`
    Directive(Directive),
    /// Field and temporaries: `pub field: Type`
    Field(Field),
}

impl Entry {
    fn as_kind(&self, p: impl FnOnce(&FieldKind) -> bool) -> Option<&Field> {
        match self {
            Entry::Field(field) if p(&field.kind) => Some(field),
            _ => None,
        }
    }
    pub fn as_field(&self) -> Option<&Field> {
        self.as_kind(|kind| matches!(kind, FieldKind::Field(_)))
    }
    pub fn as_temp(&self) -> Option<&Field> {
        self.as_kind(|kind| matches!(kind, FieldKind::Temp(_)))
    }
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

struct ResultVec<T, E>(Result<Vec<T>, E>);

impl<T, E> Default for ResultVec<T, E> {
    fn default() -> Self { ResultVec(Ok(Vec::new())) }
}

impl<T, E> Extend<Result<T, E>> for ResultVec<T, E> {
    fn extend<I: IntoIterator<Item = Result<T, E>>>(&mut self, iter: I) {
        let Ok(target) = &mut self.0 else { return; };
        for item in iter {
            match item {
                Ok(value) => target.push(value),
                Err(err) => return self.0 = Err(err),
            }
        }
    }
}

impl Parse for Field {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let (ResultVec(known_attrs), attrs) = attrs.into_iter()
            .map(|attr| KnownAttribute::new(attr, true))
            .partition_result();
        Ok(Field {
            known_attrs: known_attrs?,
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

pub enum KnownAttribute {
    Endian(Expr),
    Encode(Expr),
    Decode(Expr),
    ArgsDecl {
        direction: Direction,
        brace_token: Brace,
        fields: Punctuated<ArgFieldDecl, Token![,]>,
    },
    ArgsAssign {
        direction: Direction,
        brace_token: Brace,
        fields: Punctuated<ArgFieldAssign, Token![,]>,
    },
}

impl KnownAttribute {
    fn new(attr: Attribute, field: bool) -> Result<syn::Result<KnownAttribute>, Attribute> {
        if !attr.path().is_ident("bin_data") { return Err(attr); }
        let Meta::List(list) = attr.meta else { return Err(attr); };
        Ok(list.parse_args_with(|input: ParseStream| {
            let cmd: Ident = input.parse()?;
            fn eq_expr<T>(input: ParseStream, f: impl FnOnce(Expr) -> T) -> Result<T, Error> {
                let _: Token![=] = input.parse()?;
                input.parse().map(f)
            }
            let contents;
            match cmd.to_string().as_str() {
                "endian" => eq_expr(input, KnownAttribute::Endian),
                "encode" => eq_expr(input, KnownAttribute::Encode),
                "decode" => eq_expr(input, KnownAttribute::Decode),
                "args" if field => Ok(KnownAttribute::ArgsAssign {
                    direction: input.parse()?,
                    brace_token: braced!(contents in input),
                    fields: Punctuated::parse_terminated(&contents)?,
                }),
                "args" => Ok(KnownAttribute::ArgsDecl {
                    direction: input.parse()?,
                    brace_token: braced!(contents in input),
                    fields: Punctuated::parse_terminated(&contents)?,
                }),
                _ => Err(Error::new(cmd.span(), "unknown attribute for `bin_data`")),
            }
        }))
    }
}

#[derive(Copy, Clone)]
pub enum Direction {
    Encode,
    Decode,
    Both,
}

impl Direction {
    pub fn dispatch<T>(self, encode: &mut T, decode: &mut T, f: impl Fn(&mut T)) {
        match self {
            Direction::Encode => f(encode),
            Direction::Decode => f(decode),
            Direction::Both => {
                f(encode);
                f(decode);
            }
        }
    }
}

impl Parse for Direction {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let colon_token: Option<Token![:]> = input.parse()?;
        Ok(if colon_token.is_some() {
            let dir: Ident = input.parse()?;
            match dir.to_string().as_str() {
                "encode" => Direction::Encode,
                "decode" => Direction::Decode,
                _ => return Err(Error::new(dir.span(), "unknown direction, must be `encode` or `decode`")),
            }
        } else {
            Direction::Both
        })
    }
}

pub struct ArgFieldDecl {
    pub name: Ident,
    pub colon_token: Token![:],
    pub r#type: Type,
    pub default_value: Option<Expr>,
}

impl Parse for ArgFieldDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let colon_token = input.parse()?;
        let r#type = input.parse()?;
        let eq_token: Option<Token![=]> = input.parse()?;
        let default_value = if eq_token.is_some() { Some(input.parse()?) } else { None };
        Ok(ArgFieldDecl { name, colon_token, r#type, default_value })
    }
}

pub struct ArgFieldAssign {
    pub name: Ident,
    pub eq_token: Token![=],
    pub value: Expr,
}

impl Parse for ArgFieldAssign {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ArgFieldAssign {
            name: input.parse()?,
            eq_token: input.parse()?,
            value: input.parse()?,
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
