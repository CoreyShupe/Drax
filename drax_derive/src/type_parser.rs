use std::iter::Peekable;

use proc_macro2::token_stream::IntoIter;
use proc_macro2::{Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::{ToTokens, TokenStreamExt};
use syn::{Attribute, Type};

macro_rules! match_comma {
    ($args:ident) => {
        match $args.next() {
            None => {
                return;
            }
            Some(next) => match next {
                TokenTree::Punct(punct) => {
                    assert_eq!(',', punct.as_char());
                }
                _ => {}
            },
        }
    };
}

#[derive(Debug, Clone)]
pub(crate) enum SerialType {
    Raw(Option<Literal>),
    Json(Literal),
}

impl SerialType {
    pub fn custom_ser(&self) -> Option<(TokenStream, TokenStream)> {
        match self {
            SerialType::Raw(next) => next.as_ref().map(|literal| {
                (
                    quote::quote!(drax::extension::write_string),
                    quote::quote!(#literal),
                )
            }),
            SerialType::Json(literal) => Some((
                quote::quote!(drax::extension::write_json),
                quote::quote!(#literal),
            )),
        }
    }

    pub fn custom_de(&self) -> Option<(TokenStream, TokenStream)> {
        match self {
            SerialType::Raw(next) => next.as_ref().map(|literal| {
                (
                    quote::quote!(drax::extension::read_string),
                    quote::quote!(#literal),
                )
            }),
            SerialType::Json(literal) => Some((
                quote::quote!(drax::extension::read_json),
                quote::quote!(#literal),
            )),
        }
    }

    pub fn custom_size(&self) -> Option<TokenStream> {
        match self {
            SerialType::Raw(_) => None,
            SerialType::Json(_) => Some(quote::quote!(drax::extension::size_json)),
        }
    }
}

fn assert_next_punct(args: &mut IntoIter, character: char) {
    let next = args.next().expect("Args must contain a following =");
    if let TokenTree::Punct(next_punct) = next {
        assert_eq!(character, next_punct.as_char())
    } else {
        panic!("Did not find {} where expected", character)
    }
}

fn peek_next_punct(args: &mut Peekable<IntoIter>, character: char) {
    let next = args.next().expect("Args must contain a following =");
    if let TokenTree::Punct(next_punct) = next {
        assert_eq!(character, next_punct.as_char())
    } else {
        panic!("Did not find {} where expected", character)
    }
}

fn parse_continued_token_stream(args: &mut IntoIter) -> TokenStream {
    assert_next_punct(args, '=');
    let next = args.next().expect("Value not associated with arg.");
    if let TokenTree::Group(group) = next {
        group.stream()
    } else {
        println!("Next: {} => {:?}", next, next);
        panic!("Did not find a group following the = in an arg def.");
    }
}

fn parse_next_literal(args: &mut IntoIter) -> Literal {
    assert_next_punct(args, '=');
    let next = args.next().expect("Value not associated with arg.");
    if let TokenTree::Literal(literal) = next {
        literal
    } else {
        panic!("Did not find a group following the = in an arg def.");
    }
}

fn parse_include_statement(args: &mut IntoIter) -> IncludeStatement {
    let next: TokenTree = args.next().expect("Value not associated with arg.");
    let key_ty = if let TokenTree::Ident(ident) = next {
        ident
    } else {
        panic!("Did not find an ident following the key type in an include def.");
    };
    let next: TokenTree = args.next().expect("As not associated with arg.");
    if let TokenTree::Ident(ident) = next {
        assert_eq!(ident.to_string(), format!("as"));
    } else {
        panic!("Expected `as` after an include ty.");
    };
    let next: TokenTree = args.next().expect("Path not associated with arg.");
    let value_name = if let TokenTree::Ident(ident) = next {
        ident
    } else {
        panic!("Did not find an ident following the as in an include def.");
    };
    IncludeStatement {
        key_ty: TokenStream::from(TokenTree::from(key_ty)),
        value_name,
    }
}

#[derive(Clone)]
pub(crate) struct IncludeStatement {
    pub(crate) key_ty: TokenStream,
    pub(crate) value_name: Ident,
}

impl ToTokens for IncludeStatement {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let key_ty = &self.key_ty;
        let value_name = &self.value_name;
        tokens.append_all(quote::quote! {
            let #value_name = context.retrieve_data::<#key_ty>().map(|x| *x).unwrap();
        });
    }
}

#[derive(Default, Clone)]
pub(crate) struct StructAttributeSheet {
    pub(crate) includes: Vec<IncludeStatement>,
    pub(crate) enum_default: Option<TokenStream>,
    pub(crate) enum_key: Option<TokenStream>,
}

impl StructAttributeSheet {
    fn compile_attribute(&mut self, attribute: &Attribute) {
        let mut args: IntoIter = attribute
            .parse_args::<TokenStream>()
            .expect("Args should be present.")
            .into_iter();
        while let Some(x) = args.next() {
            match x {
                TokenTree::Ident(ident) => match ident.to_string().as_str() {
                    "include" => {
                        let mut next_stream = parse_continued_token_stream(&mut args).into_iter();
                        self.includes
                            .push(parse_include_statement(&mut next_stream))
                    }
                    "default" => self.enum_default = Some(parse_continued_token_stream(&mut args)),
                    "key" => self.enum_key = Some(parse_continued_token_stream(&mut args)),
                    _ => panic!("Unknown ident {}.", ident),
                },
                _ => panic!("Cannot define the base of the args as a non ident: {:?}", x),
            }

            match_comma!(args);
        }
    }

    pub(crate) fn create_sheet(attributes: &Vec<Attribute>) -> Self {
        let mut me = StructAttributeSheet::default();
        for x in attributes {
            if x.path.is_ident(&Ident::new("drax", Span::call_site())) {
                me.compile_attribute(x);
            }
        }
        me
    }
}

#[derive(Clone)]
pub(crate) struct TypeAttributeSheet {
    pub(crate) serial_type: SerialType,
    pub(crate) skip_if: Option<TokenStream>,
    pub(crate) default: Option<TokenStream>,
}

impl Default for TypeAttributeSheet {
    fn default() -> Self {
        Self {
            serial_type: SerialType::Raw(Option::default()),
            skip_if: Option::default(),
            default: Option::default(),
        }
    }
}

impl TypeAttributeSheet {
    fn compile_attribute(&mut self, attribute: &Attribute) {
        let mut args: IntoIter = attribute
            .parse_args::<TokenStream>()
            .expect("Args should be present.")
            .into_iter();
        while let Some(x) = args.next() {
            match x {
                TokenTree::Ident(ident) => match ident.to_string().as_str() {
                    "limit" => {
                        if let SerialType::Raw(None) = self.serial_type {
                            self.serial_type = SerialType::Raw(Some(parse_next_literal(&mut args)));
                        } else {
                            panic!("Serial type defined twice.");
                        }
                    }
                    "json" => {
                        if let SerialType::Raw(None) = self.serial_type {
                            self.serial_type = SerialType::Json(parse_next_literal(&mut args));
                        } else {
                            panic!("Serial type defined twice.");
                        }
                    }
                    "skip_if" => self.skip_if = Some(parse_continued_token_stream(&mut args)),
                    "default" => self.default = Some(parse_continued_token_stream(&mut args)),
                    _ => panic!("Unknown ident {}.", ident),
                },
                _ => panic!("Cannot define the base of the args as a non ident: {:?}", x),
            }

            match_comma!(args);
        }
    }

    pub(crate) fn create_sheet(attributes: &Vec<Attribute>) -> Self {
        let mut me = TypeAttributeSheet::default();
        for x in attributes {
            if x.path.is_ident(&Ident::new("drax", Span::call_site())) {
                me.compile_attribute(x);
            }
        }
        me
    }
}

#[derive(Clone)]
pub(crate) struct WrappedType {
    pub(crate) expanded_tokens: TokenStream,
    pub(crate) raw_type: RawType,
}

#[derive(Clone)]
pub(crate) enum RawType {
    VarInt,
    VarLong,
    SizedVec(Box<WrappedType>),
    Maybe(Box<WrappedType>),
    Vec(Box<WrappedType>),
    Option(Box<WrappedType>),
    Primitive,
    String,
    UnknownObjectType,
    Tag,
    OptionalTag,
}

impl RawType {
    pub fn wrapped(self, stream: TokenStream) -> WrappedType {
        WrappedType {
            expanded_tokens: stream,
            raw_type: self,
        }
    }

    pub fn from_token_stream(stream: IntoIter) -> WrappedType {
        Self::internal_from_token_stream(stream.peekable()).0
    }

    fn internal_from_token_stream(
        mut stream: Peekable<IntoIter>,
    ) -> (WrappedType, Peekable<IntoIter>) {
        let mut type_stream = TokenStream::new();
        while let Some(tree) = stream.peek() {
            if let TokenTree::Punct(punct) = tree {
                if punct.as_char() == '>' {
                    return (RawType::UnknownObjectType.wrapped(type_stream), stream);
                }
            }
            let tree = stream.next().unwrap();
            type_stream.append(tree.clone());
            match tree {
                TokenTree::Ident(pop_ident) => match pop_ident.to_string().as_str() {
                    "char" => panic!("Chars are currently not encodable."),
                    "VarInt" => return (RawType::VarInt.wrapped(type_stream), stream),
                    "VarLong" => return (RawType::VarLong.wrapped(type_stream), stream),
                    "CompoundTag" => return (RawType::Tag.wrapped(type_stream), stream),
                    "SizedVec" => {
                        peek_next_punct(&mut stream, '<');
                        type_stream.append(TokenTree::Punct(Punct::new('<', Spacing::Alone)));
                        let (wrapped_next, mut stream) = Self::internal_from_token_stream(stream);
                        type_stream.append_all(wrapped_next.expanded_tokens.clone());
                        let next = RawType::SizedVec(Box::new(wrapped_next));
                        peek_next_punct(&mut stream, '>');
                        type_stream.append(TokenTree::Punct(Punct::new('>', Spacing::Alone)));
                        return (next.wrapped(type_stream), stream);
                    }
                    "Maybe" => {
                        peek_next_punct(&mut stream, '<');
                        type_stream.append(TokenTree::Punct(Punct::new('<', Spacing::Alone)));
                        let (wrapped_next, mut stream) = Self::internal_from_token_stream(stream);
                        type_stream.append_all(wrapped_next.expanded_tokens.clone());
                        let next = RawType::Maybe(Box::new(wrapped_next));
                        peek_next_punct(&mut stream, '>');
                        type_stream.append(TokenTree::Punct(Punct::new('>', Spacing::Alone)));
                        return (next.wrapped(type_stream), stream);
                    }
                    "Vec" => {
                        peek_next_punct(&mut stream, '<');
                        type_stream.append(TokenTree::Punct(Punct::new('<', Spacing::Alone)));
                        let (wrapped_next, mut stream) = Self::internal_from_token_stream(stream);
                        type_stream.append_all(wrapped_next.expanded_tokens.clone());
                        let next = RawType::Vec(Box::new(wrapped_next));
                        peek_next_punct(&mut stream, '>');
                        type_stream.append(TokenTree::Punct(Punct::new('>', Spacing::Alone)));
                        return (next.wrapped(type_stream), stream);
                    }
                    "Option" => {
                        peek_next_punct(&mut stream, '<');
                        type_stream.append(TokenTree::Punct(Punct::new('<', Spacing::Alone)));
                        let (wrapped_next, mut stream) = Self::internal_from_token_stream(stream);
                        type_stream.append_all(wrapped_next.expanded_tokens.clone());
                        let next: RawType = if matches!(wrapped_next.raw_type, RawType::Tag) {
                            RawType::OptionalTag
                        } else {
                            RawType::Option(Box::new(wrapped_next))
                        };
                        peek_next_punct(&mut stream, '>');
                        type_stream.append(TokenTree::Punct(Punct::new('>', Spacing::Alone)));
                        return (next.wrapped(type_stream), stream);
                    }
                    "bool" | "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64"
                    | "u128" | "i128" | "f32" | "f64" => {
                        return (RawType::Primitive.wrapped(type_stream), stream)
                    }
                    "String" => return (RawType::String.wrapped(type_stream), stream),
                    _ => (),
                },
                TokenTree::Punct(punct) => {
                    if punct.as_char() == '<' {
                        while let Some(expecting_next) = stream.next() {
                            type_stream.append(expecting_next.clone());
                            if let TokenTree::Punct(punct) = expecting_next {
                                if punct.as_char() == '>' {
                                    break;
                                }
                            }
                        }
                        return (RawType::UnknownObjectType.wrapped(type_stream), stream);
                    }
                }
                _ => panic!("Unsupported token during type definition."),
            }
        }
        return (RawType::UnknownObjectType.wrapped(type_stream), stream);
    }

    pub(crate) fn normalize_type(syn_type: &Type) -> WrappedType {
        match syn_type {
            Type::Path(type_path) => {
                Self::from_token_stream(type_path.path.to_token_stream().into_iter())
            }
            _ => panic!("Unexpected syn type. Drax does not support this."),
        }
    }
}

pub(crate) fn create_mapping(from_expr: TokenStream, to: Ident, raw: &WrappedType) -> TokenStream {
    match &raw.raw_type {
        RawType::VarInt | RawType::VarLong | RawType::Primitive => {
            quote::quote!(let #to = #from_expr;)
        }
        _ => quote::quote!(let #to = &#from_expr;),
    }
}

pub(crate) fn create_type_ser(
    ident: &Ident,
    raw: &WrappedType,
    sheet: &TypeAttributeSheet,
) -> TokenStream {
    match &raw.raw_type {
        RawType::VarInt => {
            quote::quote!(drax::extension::write_var_int_sync(#ident, context, writer)?;)
        }
        RawType::VarLong => {
            quote::quote!(drax::extension::write_var_long_sync(#ident, context, writer)?;)
        }
        RawType::SizedVec(inner) => match (**inner).raw_type {
            RawType::Primitive => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_ser = create_type_ser(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        drax::extension::write_var_int_sync(#ident.len().try_into()?, context, writer)?;
                        for #next_ident in #ident {
                            let #next_ident = *#next_ident;
                            #inner_type_ser
                        }
                    }
                }
            }
            _ => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_ser = create_type_ser(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        drax::extension::write_var_int_sync(#ident.len().try_into()?, context, writer)?;
                        for #next_ident in #ident {
                            #inner_type_ser
                        }
                    }
                }
            }
        },
        RawType::Maybe(inner) => match (**inner).raw_type {
            RawType::Primitive => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_ser = create_type_ser(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        drax::transport::DraxTransport::write_to_transport(&#ident.is_some(), context, writer)?;
                        if let Some(#next_ident) = #ident.as_ref() {
                            let #next_ident = *#next_ident;
                            #inner_type_ser
                        }
                    }
                }
            }
            _ => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_ser = create_type_ser(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        drax::transport::DraxTransport::write_to_transport(&#ident.is_some(), context, writer)?;
                        if let Some(#next_ident) = #ident.as_ref() {
                            #inner_type_ser
                        }
                    }
                }
            }
        },
        RawType::Vec(inner) => match (**inner).raw_type {
            RawType::Primitive => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_ser = create_type_ser(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        for #next_ident in #ident {
                            let #next_ident = *#next_ident;
                            #inner_type_ser
                        }
                    }
                }
            }
            _ => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_ser = create_type_ser(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        for #next_ident in #ident {
                            #inner_type_ser
                        }
                    }
                }
            }
        },
        RawType::Option(inner) => match (**inner).raw_type {
            RawType::Primitive => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_ser = create_type_ser(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        if let Some(#next_ident) = #ident.as_ref() {
                            let #next_ident = *#next_ident;
                            #inner_type_ser
                        }
                    }
                }
            }
            _ => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_ser = create_type_ser(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        if let Some(#next_ident) = #ident.as_ref() {
                            #inner_type_ser
                        }
                    }
                }
            }
        },
        RawType::Primitive => {
            quote::quote!(drax::transport::DraxTransport::write_to_transport(&#ident, context, writer)?;)
        }
        RawType::String => match sheet.serial_type.custom_ser() {
            None => {
                quote::quote!(drax::extension::write_string(32767, #ident, context, writer)?;)
            }
            Some((custom, follower)) => {
                quote::quote!(#custom(#follower, #ident, context, writer)?;)
            }
        },
        RawType::UnknownObjectType => match sheet.serial_type.custom_ser() {
            None => {
                quote::quote!(drax::transport::DraxTransport::write_to_transport(#ident, context, writer)?;)
            }
            Some((custom, follower)) => {
                quote::quote!(#custom(#follower, #ident, context, writer)?;)
            }
        },
        RawType::Tag => quote::quote!(drax::nbt::write_nbt(#ident, writer)?;),
        RawType::OptionalTag => quote::quote!(drax::nbt::write_optional_nbt(#ident)?;),
    }
}

pub(crate) fn create_type_sizer(
    ident: &Ident,
    raw: &WrappedType,
    sheet: &TypeAttributeSheet,
) -> TokenStream {
    match &raw.raw_type {
        RawType::VarInt => {
            quote::quote!(size += drax::extension::size_var_int(#ident, context)?;)
        }
        RawType::VarLong => {
            quote::quote!(size += drax::extension::size_var_long(#ident, context)?;)
        }
        RawType::SizedVec(inner) => match (**inner).raw_type {
            RawType::Primitive => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_sizer = create_type_sizer(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        size += drax::extension::size_var_int(#ident.len().try_into()?, context)?;
                        for #next_ident in #ident {
                            let #next_ident = *#next_ident;
                            #inner_type_sizer
                        }
                    }
                }
            }
            _ => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_sizer = create_type_sizer(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        size += drax::extension::size_var_int(#ident.len().try_into()?, context)?;
                        for #next_ident in #ident {
                            #inner_type_sizer
                        }
                    }
                }
            }
        },
        RawType::Maybe(inner) => match (**inner).raw_type {
            RawType::Primitive => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_sizer = create_type_sizer(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        size += drax::transport::DraxTransport::precondition_size(&#ident.is_some(), context)?;
                        if let Some(#next_ident) = #ident.as_ref() {
                            let #next_ident = *#next_ident;
                            #inner_type_sizer
                        }
                    }
                }
            }
            _ => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_sizer = create_type_sizer(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        size += drax::transport::DraxTransport::precondition_size(&#ident.is_some(), context)?;
                        if let Some(#next_ident) = #ident.as_ref() {
                            #inner_type_sizer
                        }
                    }
                }
            }
        },
        RawType::Vec(inner) => match (**inner).raw_type {
            RawType::Primitive => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_sizer = create_type_sizer(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        for #next_ident in #ident {
                            let #next_ident = *#next_ident;
                            #inner_type_sizer
                        }
                    }
                }
            }
            _ => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_sizer = create_type_sizer(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        for #next_ident in #ident {
                            #inner_type_sizer
                        }
                    }
                }
            }
        },
        RawType::Option(inner) => match (**inner).raw_type {
            RawType::Primitive => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_sizer = create_type_sizer(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        if let Some(#next_ident) = #ident.as_ref() {
                            let #next_ident = *#next_ident;
                            #inner_type_sizer
                        }
                    }
                }
            }
            _ => {
                let next_ident = Ident::new("next", Span::call_site());
                let inner_type_sizer = create_type_sizer(&next_ident, inner, sheet);
                quote::quote! {
                    {
                        if let Some(#next_ident) = #ident.as_ref() {
                            #inner_type_sizer
                        }
                    }
                }
            }
        },
        RawType::Primitive => {
            quote::quote!(size += drax::transport::DraxTransport::precondition_size(&#ident, context)?;)
        }
        RawType::String => match sheet.serial_type.custom_size() {
            None => {
                quote::quote!(size += drax::extension::size_string(#ident, context)?;)
            }
            Some(custom) => {
                quote::quote!(size += #custom(#ident, context)?;)
            }
        },
        RawType::UnknownObjectType => match sheet.serial_type.custom_size() {
            None => {
                quote::quote!(size += drax::transport::DraxTransport::precondition_size(#ident, context)?;)
            }
            Some(custom) => {
                quote::quote!(size += #custom(#ident, context)?;)
            }
        },
        RawType::Tag => quote::quote!(size += drax::nbt::size_nbt(#ident);),
        RawType::OptionalTag => quote::quote!(size += drax::nbt::size_optional_nbt(#ident);),
    }
}

pub(crate) fn create_type_de(
    ident: &Ident,
    raw: &WrappedType,
    sheet: &TypeAttributeSheet,
) -> TokenStream {
    match &raw.raw_type {
        RawType::VarInt => {
            quote::quote!(drax::extension::read_var_int_sync(context, reader)?)
        }
        RawType::VarLong => {
            quote::quote!(drax::extension::read_var_long_sync(context, reader)?)
        }
        RawType::SizedVec(inner) => {
            let next_ident = Ident::new("next", Span::call_site());
            let inner_type_de = create_type_de(&next_ident, inner, sheet);
            quote::quote! {
                {
                    let length = drax::extension::read_var_int_sync(context, reader)?;
                    let mut #ident = Vec::with_capacity(length as usize);
                    for _ in 0..length {
                        let #next_ident = {
                            #inner_type_de
                        };
                        #ident.push(#next_ident);
                    }
                    #ident
                }
            }
        }
        RawType::Maybe(inner) => {
            let next_ident = Ident::new("next", Span::call_site());
            let inner_type_de = create_type_de(&next_ident, inner, sheet);
            quote::quote! {
                {
                    let has_next = <bool as drax::transport::DraxTransport>::read_from_transport(context, reader)?;
                    if has_next {
                        Some(#inner_type_de)
                    } else {
                        None
                    }
                }
            }
        }
        RawType::Vec(inner) => {
            let next_ident = Ident::new("next", Span::call_site());
            let inner_type_de = create_type_de(&next_ident, inner, sheet);
            quote::quote! {
                {
                    let mut #ident = Vec::new();
                    let mut full_read = Vec::new();
                    reader.read_to_end(&mut full_read)?;
                    let len = #ident.len();
                    let mut cursor = std::io::Cursor::new(full_read);

                    while cursor.position() as usize != len {
                        let #next_ident = {
                            #inner_type_de
                        };
                        #ident.push(#next_ident);
                    }
                    #ident
                }
            }
        }
        RawType::Option(inner) => {
            let next_ident = Ident::new("next", Span::call_site());
            let inner_type_de = create_type_de(&next_ident, inner, sheet);
            quote::quote! {
                Some(#inner_type_de)
            }
        }
        RawType::Primitive => {
            quote::quote!(drax::transport::DraxTransport::read_from_transport(
                context, reader
            )?)
        }
        RawType::String => match sheet.serial_type.custom_de() {
            None => {
                quote::quote!(drax::extension::read_string(32767, context, reader)?)
            }
            Some((custom, follower)) => {
                quote::quote!(#custom(#follower, context, reader)?)
            }
        },
        RawType::UnknownObjectType => match sheet.serial_type.custom_de() {
            None => {
                quote::quote!(drax::transport::DraxTransport::read_from_transport(
                    context, reader
                )?)
            }
            Some((custom, follower)) => {
                quote::quote!(#custom(#follower, context, reader)?)
            }
        },
        RawType::Tag => match sheet.serial_type.custom_de() {
            Some((_, lim)) => {
                quote::quote! {
                    {
                        match drax::nbt::read_nbt(reader, #lim)? {
                            Some(tag) => tag,
                            None => return drax::transport::Error::cause("Invalid empty tag when tag expected."),
                        }
                    }
                }
            }
            None => {
                quote::quote! {
                    {
                        match drax::nbt::read_nbt(reader, 0x200000u64)? {
                            Some(tag) => tag,
                            None => return drax::transport::Error::cause("Invalid empty tag when tag expected."),
                        }
                    }
                }
            }
        },
        RawType::OptionalTag => match sheet.serial_type.custom_de() {
            Some((_, lim)) => quote::quote!(drax::nbt::read_nbt(reader, #lim)?),
            None => quote::quote!(drax::nbt::read_nbt(reader, 0x200000u64)?),
        },
    }
}
