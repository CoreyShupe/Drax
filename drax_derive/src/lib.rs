pub(crate) mod bitmap;
pub(crate) mod r#enum;
pub(crate) mod fields;
mod nbt;
pub(crate) mod r#struct;
pub(crate) mod type_parser;

use proc_macro::TokenStream;
use syn::{Data, DeriveInput};

#[proc_macro_derive(DraxTransport, attributes(drax))]
pub fn derive_drax_transport(item: TokenStream) -> TokenStream {
    let derive_input = syn::parse_macro_input!(item as DeriveInput);
    let x = match derive_input.data {
        Data::Struct(ref data_struct) => r#struct::expand_drax_struct(&derive_input, data_struct),
        Data::Enum(ref data_enum) => r#enum::expand_drax_enum(&derive_input, data_enum),
        Data::Union(_) => unimplemented!(),
    };
    TokenStream::from(x)
}

#[proc_macro_derive(BitMapTransport)]
pub fn derive_bit_map_transport(item: TokenStream) -> TokenStream {
    let derive_input = syn::parse_macro_input!(item as DeriveInput);
    let x = match derive_input.data {
        Data::Struct(ref data_struct) => bitmap::expand_serial_bitmap(&derive_input, data_struct),
        _ => unimplemented!(),
    };
    TokenStream::from(x)
}
