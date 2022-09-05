#[cfg(feature = "footprints")]
use crate::transport::Footprint;
use crate::transport::{Error, TransportProcessorContext};
use std::io::{Cursor, Read};

pub struct PacketFrame {
    packet_id: i32,
    data: Vec<u8>,
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
    fn create_uncompressed_packet(
        context: &mut TransportProcessorContext,
        frame: PacketFrame,
    ) -> crate::transport::Result<Vec<u8>> {
        let PacketFrame { packet_id, data } = frame;

        let packet_id_size = crate::extension::size_var_int(packet_id, context)?;
        let mut var_int_bytes = Vec::with_capacity(packet_id_size);
        crate::extension::write_var_int_sync(packet_id, context, &mut var_int_bytes)?;
        Ok([var_int_bytes, data].concat())
    }

    fn create_compressed_packet_frame(
        &self,
        context: &mut TransportProcessorContext,
        frame: PacketFrame,
    ) -> crate::transport::Result<CompressedPacketFrame> {
        #[cfg(feature = "footprints")]
        context.mark(Footprint::note_struct(
            "TryFrom<PacketFrame> -> CompressedPacketFrame",
        ));

        let extra_size = crate::extension::size_var_int(frame.packet_id, context)?;
        let mut var_int_bytes = Vec::with_capacity(extra_size);
        crate::extension::write_var_int_sync(frame.packet_id, context, &mut var_int_bytes)?;
        let data = [var_int_bytes, frame.data].concat();
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
        #[cfg(feature = "footprints")]
        context.mark(Footprint::note_type("FrameEncoder"));

        if cfg!(feature = "compression") {
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
            FrameEncoder::create_uncompressed_packet(context, input)
        }
    }
}

pub struct FrameDecoder {
    #[cfg(feature = "compression")]
    compression_threshold: isize,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        #[cfg(feature = "compression")]
        return Self {
            compression_threshold: -1,
        };
        #[cfg(not(feature = "compression"))]
        Self {}
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

    fn read_raw_packet(
        context: &mut TransportProcessorContext,
        data: Vec<u8>,
    ) -> crate::transport::Result<PacketFrame> {
        let mut data_cursor = Cursor::new(data);
        let packet_id = crate::extension::read_var_int_sync(context, &mut data_cursor)?;
        Ok(PacketFrame {
            packet_id,
            data: Vec::from(data_cursor.remaining_slice()),
        })
    }

    fn decompress_frame(
        context: &mut TransportProcessorContext,
        frame: CompressedPacketFrame,
    ) -> crate::transport::Result<PacketFrame> {
        #[cfg(feature = "footprints")]
        context.mark(Footprint::note_struct(
            "TryFrom<CompressedPacketFrame> -> PacketFrame",
        ));

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
        let mut data_cursor = Cursor::new(data);
        let packet_id = crate::extension::read_var_int_sync(context, &mut data_cursor)?;
        Ok(PacketFrame {
            packet_id,
            data: Vec::from(data_cursor.remaining_slice()),
        })
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
        #[cfg(feature = "footprints")]
        context.mark(Footprint::note_type("FrameDecoder"));

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
        FrameDecoder::read_raw_packet(context, input)
    }
}
