use super::type_parser::*;
use proc_macro2::{Delimiter, Group, Punct, Spacing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::TokenStreamExt;
use syn::{DataStruct, DeriveInput, Fields};

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

    let drax_fields = super::fields::from_fields(&data.fields);
    let named = matches!(&data.fields, syn::Fields::Named(_));

    if drax_fields.is_empty() {
        return quote::quote! {
            impl drax::transport::DraxTransport for #ident {
                fn write_to_transport(
                    &self,
                    context: &mut drax::transport::TransportProcessorContext,
                    writer: &mut Vec<u8>,
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
        };
    }

    let mut creator_group = TokenStream::new();
    for (idx, drax_field) in drax_fields.iter().enumerate() {
        let ident = drax_field.field_ident.clone();
        creator_group.append(ident.clone());
        creator_group.append(Punct::new(',', Spacing::Alone));

        if named {
            mappings.push(drax_field.mapping(quote::quote!(self.#ident)));
        } else {
            let idx = syn::Index::from(idx);
            mappings.push(drax_field.mapping(quote::quote!(self.#idx)));
        }
        ser.push(drax_field.ser());
        de.push(drax_field.de());
        size.push(drax_field.size());
    }
    creator.append(Group::new(
        if matches!(&data.fields, Fields::Named(_)) {
            Delimiter::Brace
        } else {
            Delimiter::Parenthesis
        },
        creator_group,
    ));

    quote::quote! {
        impl drax::transport::DraxTransport for #ident {
            fn write_to_transport(
                &self,
                context: &mut drax::transport::TransportProcessorContext,
                writer: &mut Vec<u8>,
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
    }
}
