use crate::type_parser::{
    create_mapping, create_type_de, create_type_ser, create_type_sizer, RawType,
    TypeAttributeSheet, WrappedType,
};
use proc_macro2::{Ident, Span, TokenStream};
use syn::Fields;

#[derive(Clone)]
pub struct DraxField {
    pub(crate) field_ident: Ident,
    sheet: TypeAttributeSheet,
    pub(crate) type_ref: WrappedType,
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
        let self_info = &self.type_ref.expanded_tokens;
        let de = create_type_de(ident, &self.type_ref, &self.sheet);
        match &self.sheet.skip_if {
            None => quote::quote!(let #ident: #self_info = { #de };),
            Some(skip_req) => {
                let otherwise = self
                    .sheet
                    .default
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| quote::quote!(Default::default()));
                quote::quote! {
                    let #ident: #self_info = if !{ #skip_req } {
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

pub fn from_fields(fields: &Fields) -> Vec<DraxField> {
    match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|field| {
                let ident = field.ident.as_ref().cloned().unwrap();
                DraxField {
                    field_ident: ident,
                    sheet: TypeAttributeSheet::create_sheet(&field.attrs),
                    type_ref: RawType::normalize_type(&field.ty),
                }
            })
            .collect(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let ident = Ident::new(&format!("__v{}", index), Span::call_site());
                DraxField {
                    field_ident: ident,
                    sheet: TypeAttributeSheet::create_sheet(&field.attrs),
                    type_ref: RawType::normalize_type(&field.ty),
                }
            })
            .collect(),
        Fields::Unit => Vec::new(),
    }
}
