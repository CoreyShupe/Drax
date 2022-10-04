pub(crate) mod r#enum;
pub(crate) mod fields;
pub(crate) mod r#struct;
pub(crate) mod type_parser;

use proc_macro::TokenStream;
use syn::{Data, DeriveInput};

#[proc_macro_derive(DraxTransport, attributes(drax))]
pub fn derive_drax_transport(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_input = syn::parse_macro_input!(item as DeriveInput);
    let x = match derive_input.data {
        Data::Struct(ref data_struct) => r#struct::expand_drax_struct(&derive_input, data_struct),
        Data::Enum(ref data_enum) => r#enum::expand_drax_enum(&derive_input, data_enum),
        Data::Union(_) => unimplemented!(),
    };
    TokenStream::from(x)
}
