use proc_macro2::TokenStream;
use syn::{DataStruct, DeriveInput};

pub fn expand_serial_bitmap(derive_input: &DeriveInput, syn_struct: &DataStruct) -> TokenStream {
    let struct_ident = &derive_input.ident;

    let (mut ser, mut de, mut make) = (Vec::new(), Vec::new(), Vec::new());

    let mut bit_marker = 1u8;

    for field in syn_struct.fields.iter() {
        let field_ident = field.ident.as_ref().unwrap();
        ser.push(quote::quote!(if self.#field_ident { by |= #bit_marker; }));
        de.push(quote::quote!(let #field_ident = (by & #bit_marker) != 0;));
        make.push(quote::quote!(#field_ident,));
        bit_marker *= 2;
    }

    quote::quote! {
        impl drax::transport::DraxTransport for #struct_ident {
            fn write_to_transport(
                &self,
                context: &mut drax::transport::TransportProcessorContext,
                writer: &mut std::io::Cursor<Vec<u8>>,
            ) -> drax::transport::Result<()> {
                let mut by = 0u8;
                #(#ser)*
                u8::write_to_transport(&by, context, writer)
            }

            fn read_from_transport<R: std::io::Read>(
                context: &mut drax::transport::TransportProcessorContext,
                reader: &mut R,
            ) -> drax::transport::Result<Self>
            where
            Self: Sized {
                let by = u8::read_from_transport(context, reader)?;
                #(#de)*
                Ok(Self { #(#make)* })
            }

            fn precondition_size(&self, context: &mut drax::transport::TransportProcessorContext) -> drax::transport::Result<usize> {
                Ok(1)
            }
        }
    }
}
