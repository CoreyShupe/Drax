use drax::link;
use drax::transport::buffered_writer::{FrameSizeAppender, GenericWriter};
use drax::transport::frame::FrameEncoder;
use drax::transport::pipeline::ChainProcessor;
use drax::transport::DraxTransport;
use drax::VarInt;
use std::io::Cursor;
use tokio::io::AsyncWriteExt;

#[derive(drax_derive::DraxTransport)]
pub struct StatusRequest;

#[derive(drax_derive::DraxTransport, Debug)]
pub struct StatusResponse(#[drax(json = 32767)] serde_json::Value);

#[derive(drax_derive::DraxTransport)]
#[drax(key = {VarInt})]
pub enum NextState {
    Handshake,
    Status,
    Login,
}

#[derive(drax_derive::DraxTransport)]
pub struct Handshake {
    pub protocol_version: VarInt,
    #[drax(limit = 255)]
    pub server_address: String,
    pub server_port: u16,
    pub next_state: NextState,
}

#[derive(drax_derive::DraxTransport, Debug)]
pub struct Pong {
    pub start_time: i64,
}

impl From<Ping> for Pong {
    fn from(ping: Ping) -> Self {
        Self {
            start_time: ping.start_time,
        }
    }
}

#[derive(drax_derive::DraxTransport)]
pub struct Ping {
    pub start_time: i64,
}

impl From<Pong> for Ping {
    fn from(pong: Pong) -> Self {
        Self {
            start_time: pong.start_time,
        }
    }
}

#[derive(drax_derive::DraxTransport)]
#[drax(key = {match VarInt})]
pub enum HandshakeRegistryPacket {
    Handshake(Handshake),
}

#[derive(drax_derive::DraxTransport)]
#[drax(key = {match VarInt})]
pub enum ServerBoundStatusPacket {
    StatusRequest(StatusRequest),
    Ping(Ping),
}

#[derive(drax_derive::DraxTransport)]
#[drax(key = {match VarInt})]
pub enum ClientBoundStatusPacket {
    StatusResponse(StatusResponse),
    Pong(Pong),
}

pub struct StatusResponseChainProcessor;
impl ChainProcessor for StatusResponseChainProcessor {
    type Input = drax::transport::frame::PacketFrame;
    type Output = ClientBoundStatusPacket;

    fn process(
        &self,
        context: &mut drax::transport::TransportProcessorContext,
        input: Self::Input,
    ) -> drax::transport::Result<Self::Output> {
        let mut cursor = Cursor::new(input.data);
        <ClientBoundStatusPacket as DraxTransport>::read_from_transport(context, &mut cursor)
    }
}

const BUFFER_CAPACITY: usize = 2097154; // static value from wiki.vg

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    let client = tokio::net::TcpStream::connect("mh-prd.minehut.com:25565").await?;
    let (mut read, mut write) = client.into_split();

    let full_chain = link!(
        drax::transport::frame::FrameDecoder::new(-1),
        StatusResponseChainProcessor
    );
    let write_pipeline = link!(GenericWriter, FrameEncoder::new(-1), FrameSizeAppender);

    let buffer = drax::prelude::BytesMut::with_capacity(BUFFER_CAPACITY);
    let mut drax_transport_pipeline =
        drax::transport::buffered_reader::DraxTransportPipeline::new(Box::new(full_chain), buffer);

    let mut context = drax::transport::TransportProcessorContext::new();

    let (handshake, status_req) = (
        HandshakeRegistryPacket::Handshake(Handshake {
            protocol_version: 760,
            server_address: "mh-prd.minehut.com".to_string(),
            server_port: 25565,
            next_state: NextState::Status,
        }),
        ServerBoundStatusPacket::StatusRequest(StatusRequest),
    );
    let starter = [
        write_pipeline.process(&mut context, Box::new(handshake))?,
        write_pipeline.process(&mut context, Box::new(status_req))?,
    ]
    .concat();

    write.write_all(&starter).await?; // handshake + status req has been sent, now we wait for a status response

    let status_response = drax_transport_pipeline
        .read_transport_packet(&mut context, &mut read)
        .await?;

    match status_response {
        ClientBoundStatusPacket::StatusResponse(response) => {
            println!("Status response: {:#?}", response)
        }
        ClientBoundStatusPacket::Pong(_) => panic!("Unexpected `Pong` in this state."),
    }

    let ping = ServerBoundStatusPacket::Ping(Ping {
        start_time: std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)?
            .as_millis() as i64,
    });
    let ping_data = write_pipeline.process(&mut context, Box::new(ping))?;
    write.write_all(&ping_data).await?;

    let status_response = drax_transport_pipeline
        .read_transport_packet(&mut context, &mut read)
        .await?;
    match status_response {
        ClientBoundStatusPacket::StatusResponse(_) => {
            panic!("Unexpected `StatusResponse` in this state.")
        }
        ClientBoundStatusPacket::Pong(pong) => println!("Received pong: {:?}", pong),
    }

    Ok(())
}
