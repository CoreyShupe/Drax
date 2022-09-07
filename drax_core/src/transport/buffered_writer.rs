pub struct FrameSizeAppender;
impl super::pipeline::ChainProcessor for FrameSizeAppender {
    type Input = Vec<u8>;
    type Output = Vec<u8>;

    fn process(
        &mut self,
        context: &mut super::TransportProcessorContext,
        input: Self::Input,
    ) -> super::Result<Self::Output> {
        let size = input.len() as i32;
        let mut header_buffer = Vec::with_capacity(crate::extension::size_var_int(size, context)?);
        crate::extension::write_var_int_sync(size, context, &mut header_buffer)?;
        Ok([header_buffer, input].concat())
    }
}

pub struct GenericWriter;
impl super::pipeline::ChainProcessor for GenericWriter {
    type Input = Box<dyn super::DraxTransport>;
    type Output = super::frame::PacketFrame;

    fn process(
        &mut self,
        context: &mut super::TransportProcessorContext,
        input: Self::Input,
    ) -> super::Result<Self::Output>
    where
        Self::Input: Sized,
    {
        let mut packet_buffer = Vec::with_capacity(input.precondition_size(context)?);
        input.write_to_transport(context, &mut packet_buffer)?;
        Ok(super::frame::PacketFrame {
            data: packet_buffer,
        })
    }
}
