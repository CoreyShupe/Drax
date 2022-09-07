use crate::transport::{Error, TransportProcessorContext};
use std::io::{Cursor, Read};

pub struct PacketFrame {
    pub data: Vec<u8>,
}

#[cfg(feature = "compression")]
struct CompressedPacketFrame {
    decompressed_data_length: i32,
    compressed_data: Cursor<Vec<u8>>,
}

use crate::transport::pipeline::ChainProcessor;
#[cfg(feature = "compression")]
use flate2::{
    bufread::{ZlibDecoder, ZlibEncoder},
    Compression,
};

pub struct FrameEncoder {
    #[cfg(feature = "compression")]
    compression_threshold: isize,
}

impl FrameEncoder {
    #[cfg(feature = "compression")]
    pub fn new(compression_threshold: isize) -> Self {
        Self {
            compression_threshold,
        }
    }

    #[cfg(not(feature = "compression"))]
    pub fn new() -> Self {
        Self {}
    }

    fn create_uncompressed_packet(frame: PacketFrame) -> crate::transport::Result<Vec<u8>> {
        let PacketFrame { data } = frame;
        Ok(data)
    }

    fn create_compressed_packet_frame(
        &self,
        _context: &mut TransportProcessorContext,
        frame: PacketFrame,
    ) -> crate::transport::Result<CompressedPacketFrame> {
        let data = frame.data;
        let true_data_len = data.len();
        if data.len() < self.compression_threshold as usize {
            Ok(CompressedPacketFrame {
                decompressed_data_length: 0,
                compressed_data: Cursor::new(data),
            })
        } else {
            let mut encoder = ZlibEncoder::new(data.as_slice(), Compression::default());
            let mut compressed = Vec::new();
            encoder.read_to_end(&mut compressed)?;
            Ok(CompressedPacketFrame {
                decompressed_data_length: true_data_len.try_into()?,
                compressed_data: Cursor::new(compressed),
            })
        }
    }
}

impl ChainProcessor for FrameEncoder {
    type Input = PacketFrame;
    type Output = Vec<u8>;

    fn process(
        &mut self,
        context: &mut TransportProcessorContext,
        input: Self::Input,
    ) -> crate::transport::Result<Self::Output> {
        if cfg!(feature = "compression") && self.compression_threshold >= 0 {
            let CompressedPacketFrame {
                decompressed_data_length,
                compressed_data,
            } = self.create_compressed_packet_frame(context, input)?;
            let compressed = compressed_data.into_inner();
            let decompressed_data_length_size =
                crate::extension::size_var_int(decompressed_data_length, context)?;

            let mut data = Vec::with_capacity(decompressed_data_length_size);
            crate::extension::write_var_int_sync(decompressed_data_length, context, &mut data)?;
            Ok([data, compressed].concat())
        } else {
            FrameEncoder::create_uncompressed_packet(input)
        }
    }
}

pub struct FrameDecoder {
    #[cfg(feature = "compression")]
    compression_threshold: isize,
}

#[cfg(feature = "compression")]
impl Default for FrameDecoder {
    fn default() -> Self {
        return Self {
            compression_threshold: -1,
        };
    }
}

impl FrameDecoder {
    #[cfg(feature = "compression")]
    pub fn new(compression_threshold: isize) -> Self {
        Self {
            compression_threshold,
        }
    }

    #[cfg(not(feature = "compression"))]
    pub fn new() -> Self {
        Self {}
    }

    fn create_raw_packet(data: Vec<u8>) -> crate::transport::Result<PacketFrame> {
        Ok(PacketFrame { data })
    }

    fn decompress_frame(
        _context: &mut TransportProcessorContext,
        frame: CompressedPacketFrame,
    ) -> crate::transport::Result<PacketFrame> {
        let data_length = frame.decompressed_data_length as usize;
        let data = if data_length == 0 {
            frame.compressed_data.into_inner()
        } else {
            let mut preconditioned_data = Vec::with_capacity(data_length);
            let actual_decoded =
                ZlibDecoder::new(frame.compressed_data).read_to_end(&mut preconditioned_data)?;
            if actual_decoded != data_length {
                return Error::cause(format!(
                    "Actual decoded {} is not the same as data length {}",
                    actual_decoded, data_length
                ));
            }
            preconditioned_data
        };
        Ok(PacketFrame { data })
    }
}

impl ChainProcessor for FrameDecoder {
    type Input = Vec<u8>;
    type Output = PacketFrame;

    fn process(
        &mut self,
        context: &mut TransportProcessorContext,
        input: Self::Input,
    ) -> crate::transport::Result<Self::Output> {
        #[cfg(feature = "compression")]
        if self.compression_threshold >= 0 {
            let mut data_cursor = Cursor::new(input);
            let decompressed_data_length =
                crate::extension::read_var_int_sync(context, &mut data_cursor)?;
            let compressed_frame = CompressedPacketFrame {
                decompressed_data_length,
                compressed_data: data_cursor,
            };
            return FrameDecoder::decompress_frame(context, compressed_frame);
        }
        FrameDecoder::create_raw_packet(input)
    }
}
