use crate::fields::DraxField;
use crate::type_parser::{
    create_type_de, create_type_ser, create_type_sizer, RawType, StructAttributeSheet,
    TypeAttributeSheet,
};
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use syn::{DataEnum, DeriveInput, Variant};

#[derive(Clone)]
pub(crate) struct DraxVariant {
    variant_ident: Ident,
    fields: Vec<DraxField>,
    named_fields: bool,
    attribute_sheet: StructAttributeSheet,
    defined_key: TokenStream,
    key_type: RawType,
}

impl DraxVariant {
    pub fn from_variant(variant: &Variant, ordinal: usize, key_type: RawType) -> Self {
        let fields = &variant.fields;
        let sheet = StructAttributeSheet::create_sheet(&variant.attrs);
        let defined_key = sheet.enum_key.as_ref().cloned().unwrap_or_else(|| {
            let idx = syn::Index::from(ordinal);
            quote::quote!(#idx)
        });
        Self {
            variant_ident: variant.ident.clone(),
            fields: super::fields::from_fields(fields),
            named_fields: matches!(fields, syn::Fields::Named(_)),
            attribute_sheet: sheet,
            defined_key,
            key_type,
        }
    }

    fn spec_creator(&self) -> TokenStream {
        let inner_ident = self.variant_ident.clone();
        if self.fields.is_empty() {
            return quote::quote!(Self::#inner_ident);
        }
        let creator_group: TokenStream = self
            .fields
            .iter()
            .flat_map(|field| {
                if self.named_fields {
                    vec![
                        TokenTree::from(field.field_ident.clone()),
                        TokenTree::from(Punct::new(':', Spacing::Alone)),
                        TokenTree::from(Ident::new("_", Span::call_site())),
                        TokenTree::from(Punct::new(',', Spacing::Alone)),
                    ]
                } else {
                    vec![
                        TokenTree::from(field.field_ident.clone()),
                        TokenTree::from(Punct::new(',', Spacing::Alone)),
                    ]
                }
            })
            .collect();
        let creator = if self.named_fields {
            Group::new(Delimiter::Brace, creator_group)
        } else {
            Group::new(Delimiter::Parenthesis, creator_group)
        };
        quote::quote!(Self::#inner_ident #creator)
    }

    fn arm(&self) -> TokenStream {
        let spec = self.spec_creator();
        quote::quote!(#spec =>)
    }

    pub fn ser(&self, ser_key: bool) -> TokenStream {
        let includes = &self.attribute_sheet.includes;
        let ser = self
            .fields
            .iter()
            .map(|x| match x.type_ref {
                RawType::Primitive => {
                    let ident = &x.field_ident;
                    let ser = x.ser();
                    quote::quote! {
                        let #ident = *#ident;
                        #ser
                    }
                }
                _ => x.ser(),
            })
            .collect::<Vec<TokenStream>>();

        let arm = self.arm();
        let key_ser = if ser_key {
            let key_ident = Ident::new("key", Span::call_site());
            let key_type = &self.key_type;
            let ref_ser = create_type_ser(&key_ident, key_type, &TypeAttributeSheet::default());
            let key_out = &self.defined_key;
            quote::quote! {
                {
                    let #key_ident = #key_out;
                    #ref_ser
                }
            }
        } else {
            TokenStream::new()
        };

        quote::quote! {
            #arm {
                #key_ser
                #(#includes)*
                #(#ser)*
            }
        }
    }

    pub fn raw_de(&self) -> TokenStream {
        let includes = &self.attribute_sheet.includes;
        let de = self
            .fields
            .iter()
            .map(|x| x.de())
            .collect::<Vec<TokenStream>>();

        let creator = self.spec_creator();
        quote::quote! {
            #(#includes)*
            #(#de)*
            Ok(#creator)
        }
    }

    pub fn sizer(&self, size_key: bool) -> TokenStream {
        let includes = &self.attribute_sheet.includes;
        let sizer = self
            .fields
            .iter()
            .map(|x| match x.type_ref {
                RawType::Primitive => {
                    let ident = &x.field_ident;
                    let sizer = x.size();
                    quote::quote! {
                        let #ident = *#ident;
                        #sizer
                    }
                }
                _ => x.size(),
            })
            .collect::<Vec<TokenStream>>();

        let arm = self.arm();
        let key_ser = if size_key {
            let key_type = &self.key_type;
            let key_ident = Ident::new("key", Span::call_site());
            let ref_sizer = create_type_sizer(&key_ident, key_type, &TypeAttributeSheet::default());
            let key_out = &self.defined_key;
            quote::quote! {
                {
                    let #key_ident = #key_out;
                    #ref_sizer
                }
            }
        } else {
            TokenStream::new()
        };

        quote::quote! {
            #arm {
                #key_ser
                #(#includes)*
                #(#sizer)*
            }
        }
    }
}

enum KeyType {
    Inherited(TokenStream),
    InheritedMatch(TokenStream),
    Match(TokenStream),
    RawType(TokenStream),
}

fn parse_key_type(stream: TokenStream) -> KeyType {
    let mut stream_clone_iter = stream.clone().into_iter();
    match stream_clone_iter.next() {
        None => panic!("Key must have a value defined."),
        Some(token_tree) => match token_tree {
            TokenTree::Ident(ident) => match ident.to_string().as_str() {
                "from" => KeyType::Inherited(stream_clone_iter.collect()),
                "from_match" => KeyType::InheritedMatch(stream_clone_iter.collect()),
                "match" => KeyType::Match(stream_clone_iter.collect()),
                _ => KeyType::RawType(stream),
            },
            _ => KeyType::RawType(stream),
        },
    }
}

fn variant_if_arms(
    key_ident: &Ident,
    arms: &Vec<DraxVariant>,
    default_variant: &Option<String>,
) -> TokenStream {
    let mut match_default: Option<TokenStream> = None;
    let mut match_arms: Vec<TokenStream> = Vec::with_capacity(arms.len());
    let mut first = true;

    for variant in arms.iter() {
        let raw_de = variant.raw_de();
        if default_variant
            .as_ref()
            .map(|x| x.eq(&variant.variant_ident.to_string()))
            .unwrap_or_else(|| false)
        {
            match_default = Some(quote::quote! {
                else {
                    #raw_de
                }
            });
        }
        let key = &variant.defined_key;
        if first {
            match_arms.push(quote::quote! {
                if #key == #key_ident {
                    #raw_de
                }
            });
            first = false;
        } else {
            match_arms.push(quote::quote! {
                else if #key == #key_ident {
                    #raw_de
                }
            });
        }
    }
    let match_default = match_default.unwrap_or_else(|| {
        quote::quote! {
            else {
                drax::transport::Error::cause(format!("Invalid variant key {}", #key_ident))
            }
        }
    });
    quote::quote! {
        #(#match_arms)*
        #match_default
    }
}

fn variant_match_arms(
    key_ident: &Ident,
    arms: &Vec<DraxVariant>,
    default_variant: &Option<String>,
) -> TokenStream {
    let mut match_default: Option<TokenStream> = None;
    let mut match_arms: Vec<TokenStream> = Vec::with_capacity(arms.len());
    for variant in arms.iter() {
        let raw_de = variant.raw_de();
        if default_variant
            .as_ref()
            .map(|x| x.eq(&variant.variant_ident.to_string()))
            .unwrap_or_else(|| false)
        {
            match_default = Some(quote::quote! {
                _ => {
                    #raw_de
                }
            });
        }
        let key = &variant.defined_key;
        match_arms.push(quote::quote! {
            #key => {
                #raw_de
            }
        });
    }

    let match_default = match_default.unwrap_or_else(|| {
        quote::quote! {
            _ => {
                drax::transport::Error::cause(format!("Invalid variant key {}", #key_ident))
            }
        }
    });

    quote::quote! {
        match key {
            #(#match_arms)*
            #match_default
        }
    }
}

pub fn expand_drax_enum(input: &DeriveInput, data: &DataEnum) -> TokenStream {
    let enum_ident = input.ident.clone();
    let enum_data_sheet = StructAttributeSheet::create_sheet(&input.attrs);
    let includes = &enum_data_sheet.includes;

    let default_variant = enum_data_sheet.enum_default.clone().map(|ts| {
        let mut iter = ts.into_iter();
        if let Some(TokenTree::Ident(ident)) = iter.next() {
            ident.to_string()
        } else {
            panic!("Invalid value in enum default def.");
        }
    });

    let true_key_type = parse_key_type(
        enum_data_sheet
            .enum_key
            .expect("An enum must provide a valid key."),
    );

    let enum_key_type = match &true_key_type {
        KeyType::Inherited(_) => RawType::UnknownObjectType,
        KeyType::InheritedMatch(_) => RawType::UnknownObjectType,
        KeyType::Match(ts) => RawType::from_token_stream(ts.clone().into_iter()),
        KeyType::RawType(ts) => RawType::from_token_stream(ts.clone().into_iter()),
    };

    let variants = data
        .variants
        .iter()
        .enumerate()
        .map(|(idx, variant)| DraxVariant::from_variant(variant, idx, enum_key_type.clone()))
        .collect::<Vec<DraxVariant>>();

    let (enum_deserializer, ser_key) = match true_key_type {
        KeyType::Inherited(key_ty) => {
            let key_ident = Ident::new("key", Span::call_site());
            let include_key = super::type_parser::IncludeStatement {
                key_ty,
                value_name: key_ident.clone(),
            };
            let matcher = variant_if_arms(&key_ident, &variants, &default_variant);

            (
                quote::quote! {
                    #include_key
                    #matcher
                },
                false,
            )
        }
        KeyType::InheritedMatch(key_ty) => {
            let key_ident = Ident::new("key", Span::call_site());
            let include_key = super::type_parser::IncludeStatement {
                key_ty,
                value_name: key_ident.clone(),
            };

            let matcher = variant_match_arms(&key_ident, &variants, &default_variant);

            (
                quote::quote! {
                    #include_key
                    #matcher
                },
                false,
            )
        }
        KeyType::Match(_) => {
            let key_ident = Ident::new("key", Span::call_site());
            let matcher = variant_match_arms(&key_ident, &variants, &default_variant);
            let de = create_type_de(&key_ident, &enum_key_type, &TypeAttributeSheet::default());

            (
                quote::quote! {
                    let #key_ident = {
                        #de
                    };
                    #matcher
                },
                true,
            )
        }
        KeyType::RawType(_) => {
            let key_ident = Ident::new("key", Span::call_site());
            let matcher = variant_if_arms(&key_ident, &variants, &default_variant);
            let de = create_type_de(&key_ident, &enum_key_type, &TypeAttributeSheet::default());

            (
                quote::quote! {
                    let #key_ident = {
                        #de
                    };
                    #matcher
                },
                true,
            )
        }
    };

    let sers = variants.iter().map(|variant| variant.ser(ser_key));
    let sizers = variants.iter().map(|variant| variant.sizer(ser_key));

    quote::quote! {
        impl drax::transport::DraxTransport for #enum_ident {
            fn write_to_transport(
                &self,
                context: &mut drax::transport::TransportProcessorContext,
                writer: &mut std::io::Cursor<Vec<u8>>,
            ) -> drax::transport::Result<()> {
                #(#includes)*
                match self {
                    #(#sers)*
                }
                Ok(())
            }

            fn read_from_transport<R: std::io::Read>(
                context: &mut drax::transport::TransportProcessorContext,
                reader: &mut R,
            ) -> drax::transport::Result<Self>
            where
                Self: Sized {
                #(#includes)*
                #enum_deserializer
            }

            fn precondition_size(&self, context: &mut drax::transport::TransportProcessorContext) -> drax::transport::Result<usize> {
                let mut size = 0;
                #(#includes)*
                match self {
                    #(#sizers)*
                }
                Ok(size)
            }
        }
    }
}
