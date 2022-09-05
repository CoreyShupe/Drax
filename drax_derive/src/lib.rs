pub(crate) mod r#enum;
pub(crate) mod r#struct;
pub(crate) mod type_parser;

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{Data, DeriveInput, Type};

#[proc_macro_derive(DraxTransport, attributes(drax))]
pub fn derive_drax_transport(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_input = syn::parse_macro_input!(item as DeriveInput);
    match derive_input.data {
        Data::Struct(ref data_struct) => {
            TokenStream::from(r#struct::expand_drax_struct(&derive_input, &data_struct))
        }
        Data::Enum(_) => unimplemented!(),
        Data::Union(_) => unimplemented!(),
    }
}
