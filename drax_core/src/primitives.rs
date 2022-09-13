use crate::transport::{DraxTransport, TransportProcessorContext};
use std::io::{Read, Write};

macro_rules! define_primitive {
    ($prim_type:ty, $byte_count:literal) => {
        impl DraxTransport for $prim_type {
            fn write_to_transport(
                &self,
                _context: &mut TransportProcessorContext,
                writer: &mut Vec<u8>,
            ) -> crate::transport::Result<()> {
                writer.write_all(&self.to_be_bytes())?;
                Ok(())
            }

            fn read_from_transport<R: Read>(
                _context: &mut TransportProcessorContext,
                read: &mut R,
            ) -> crate::transport::Result<Self> {
                let mut bytes = [0u8; $byte_count];
                read.read_exact(&mut bytes)?;
                Ok(<$prim_type>::from_be_bytes(bytes))
            }

            fn precondition_size(
                &self,
                _context: &mut TransportProcessorContext,
            ) -> crate::transport::Result<usize> {
                Ok($byte_count)
            }
        }
    };
}

define_primitive!(u8, 1);
define_primitive!(i8, 1);
define_primitive!(u16, 2);
define_primitive!(i16, 2);
define_primitive!(u32, 4);
define_primitive!(i32, 4);
define_primitive!(u64, 8);
define_primitive!(i64, 8);
define_primitive!(u128, 16);
define_primitive!(i128, 16);
define_primitive!(f32, 4);
define_primitive!(f64, 8);

impl DraxTransport for bool {
    fn write_to_transport(
        &self,
        _context: &mut TransportProcessorContext,
        writer: &mut Vec<u8>,
    ) -> crate::transport::Result<()> {
        writer.write_all(&[if *self { 0x1 } else { 0x0 }])?;
        Ok(())
    }

    fn read_from_transport<R: Read>(
        _context: &mut TransportProcessorContext,
        read: &mut R,
    ) -> crate::transport::Result<Self>
    where
        Self: Sized,
    {
        let mut byte = [0u8; 1];
        read.read_exact(&mut byte)?;
        Ok(byte[0] != 0x0)
    }

    fn precondition_size(
        &self,
        _context: &mut TransportProcessorContext,
    ) -> crate::transport::Result<usize> {
        Ok(1)
    }
}
