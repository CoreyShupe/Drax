use super::type_parser::*;
use drax::transport::TransportProcessorContext;
use drax::SizedVec;
use proc_macro2::{Delimiter, Group, Punct, Spacing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::TokenStreamExt;
use std::io::{Read, Write};
use syn::{DataStruct, DeriveInput, Field, Fields};

pub struct DraxField {
    field_ident: Ident,
    sheet: TypeAttributeSheet,
    type_ref: RawType,
}

impl DraxField {
    pub fn ser(&self) -> TokenStream {
        let serializer = create_type_ser(&self.field_ident, &self.type_ref, &self.sheet);
        match &self.sheet.skip_if {
            None => quote::quote!(#serializer),
            Some(skip_req) => {
                quote::quote! {
                    if !{ #skip_req } {
                        #serializer
                    }
                }
            }
        }
    }

    pub fn size(&self) -> TokenStream {
        let sizer = create_type_sizer(&self.field_ident, &self.type_ref, &self.sheet);
        match &self.sheet.skip_if {
            None => quote::quote!(#sizer),
            Some(skip_req) => {
                quote::quote! {
                    if !{ #skip_req } {
                        #sizer
                    }
                }
            }
        }
    }

    pub fn de(&self) -> TokenStream {
        let ident = &self.field_ident;
        let de = create_type_de(ident, &self.type_ref, &self.sheet);
        match &self.sheet.skip_if {
            None => quote::quote!(let #ident = { #de };),
            Some(skip_req) => {
                let otherwise = self
                    .sheet
                    .default
                    .as_ref()
                    .map(|x| x.clone())
                    .unwrap_or_else(|| quote::quote!(Default::default()));
                quote::quote! {
                    let #ident = if !{ #skip_req } {
                        #de
                    } else {
                        #otherwise
                    };
                }
            }
        }
    }

    pub fn mapping(&self, expr: TokenStream) -> TokenStream {
        create_mapping(expr, self.field_ident.clone(), &self.type_ref)
    }
}

pub fn expand_drax_struct(input: &DeriveInput, data: &DataStruct) -> TokenStream {
    let ident = &input.ident;
    let struct_attribute_sheet = StructAttributeSheet::create_sheet(&input.attrs);
    let includes = struct_attribute_sheet.includes;

    let mut mappings = Vec::with_capacity(data.fields.len());
    let mut ser = Vec::with_capacity(data.fields.len());
    let mut de = Vec::with_capacity(data.fields.len());
    let mut size = Vec::with_capacity(data.fields.len());

    let mut creator = TokenStream::new();
    creator.append(Ident::new("Self", Span::call_site()));

    match &data.fields {
        Fields::Named(named) => {
            let mut creator_group = TokenStream::new();
            for field in named.named.iter() {
                let ident = field.ident.as_ref().unwrap();
                creator_group.append(ident.clone());
                creator_group.append(Punct::new(',', Spacing::Alone));

                let drax_field = DraxField {
                    field_ident: ident.clone(),
                    sheet: TypeAttributeSheet::create_sheet(&field.attrs),
                    type_ref: RawType::normalize_type(&field.ty),
                };

                mappings.push(drax_field.mapping(quote::quote!(self.#ident)));
                ser.push(drax_field.ser());
                de.push(drax_field.de());
                size.push(drax_field.size());
            }
            creator.append(Group::new(Delimiter::Brace, creator_group));
        }
        Fields::Unnamed(unnamed) => {
            let mut creator_group = TokenStream::new();
            for (index, field) in unnamed.unnamed.iter().enumerate() {
                let stub_ident = Ident::new(&format!("__v{}", index), Span::call_site());
                creator_group.append(stub_ident.clone());
                creator_group.append(Punct::new(',', Spacing::Alone));

                let drax_field = DraxField {
                    field_ident: ident.clone(),
                    sheet: TypeAttributeSheet::create_sheet(&field.attrs),
                    type_ref: RawType::normalize_type(&field.ty),
                };

                mappings.push(drax_field.mapping(quote::quote!(self.#index)));
                ser.push(drax_field.ser());
                de.push(drax_field.de());
                size.push(drax_field.size());
            }
            creator.append(Group::new(Delimiter::Parenthesis, creator_group));
        }
        Fields::Unit => {
            return quote::quote! {
                impl drax::transport::DraxTransport for #ident {
                    fn write_to_transport<W: std::io::Write>(
                        &self,
                        context: &mut drax::transport::TransportProcessorContext,
                        writer: &mut W,
                    ) -> drax::transport::Result<()> {
                        Ok(())
                    }

                    fn read_from_transport<R: std::io::Read>(
                        context: &mut drax::transport::TransportProcessorContext,
                        reader: &mut R,
                    ) -> drax::transport::Result<Self>
                    where
                        Self: Sized {
                        Ok(Self)
                    }

                    fn precondition_size(&self, context: &mut drax::transport::TransportProcessorContext) -> drax::transport::Result<usize> {
                        Ok(0)
                    }
                }
            }
        }
    }

    let x = quote::quote! {
        impl drax::transport::DraxTransport for #ident {
            fn write_to_transport<W: std::io::Write>(
                &self,
                context: &mut drax::transport::TransportProcessorContext,
                writer: &mut W,
            ) -> drax::transport::Result<()> {
                #(#includes)*
                #(#mappings)*
                #(#ser)*
                Ok(())
            }

            fn read_from_transport<R: std::io::Read>(
                context: &mut drax::transport::TransportProcessorContext,
                reader: &mut R,
            ) -> drax::transport::Result<Self>
            where
            Self: Sized {
                #(#includes)*
                #(#de)*
                Ok(#creator)
            }

            fn precondition_size(
                &self,
                context: &mut drax::transport::TransportProcessorContext
            ) -> drax::transport::Result<usize> {
                #(#includes)*
                #(#mappings)*
                let mut size = 0;
                #(#size)*
                Ok(size)
            }
        }
    };
    println!("{}", x);
    x
}
