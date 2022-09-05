#[cfg(feature = "pipelines")]
pub mod buffered_reader;
#[cfg(feature = "encryption")]
mod encryption;
#[cfg(feature = "pipelines")]
mod frame;
#[cfg(feature = "pipelines")]
pub mod pipeline;

use bytes::BytesMut;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::io::{Read, Write};
use std::num::TryFromIntError;
use std::pin::Pin;
use std::string::FromUtf8Error;
use std::task::{Context, Poll};
use tokio::io::AsyncRead;

#[derive(Debug)]
pub enum Error {
    Unknown(Option<String>),
    TokioError(tokio::io::Error),
    TryFromIntError(TryFromIntError),
    FromUtf8Error(FromUtf8Error),
    SerdeJsonError(serde_json::Error),
}

impl Error {
    pub fn cause<T, S: Into<String>>(into: S) -> Result<T> {
        Err(Self::Unknown(Some(into.into())))
    }

    pub fn no_cause<T>() -> Result<T> {
        Err(Self::Unknown(None))
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transport Error: ")?;
        match self {
            Error::Unknown(potential_reason) => match potential_reason {
                None => write!(f, "Unknown error"),
                Some(reason) => write!(f, "Caught reason: {}", reason),
            },
            Error::TokioError(err) => write!(f, "{}", err),
            Error::TryFromIntError(err) => write!(f, "{}", err),
            Error::FromUtf8Error(err) => write!(f, "{}", err),
            Error::SerdeJsonError(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for Error {}

impl From<tokio::io::Error> for Error {
    fn from(tokio_error: tokio::io::Error) -> Self {
        Self::TokioError(tokio_error)
    }
}

impl From<TryFromIntError> for Error {
    fn from(try_from_int_error: TryFromIntError) -> Self {
        Self::TryFromIntError(try_from_int_error)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(from_utf8_error: FromUtf8Error) -> Self {
        Self::FromUtf8Error(from_utf8_error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(serde_json_error: serde_json::Error) -> Self {
        Self::SerdeJsonError(serde_json_error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "footprints")]
#[derive(Debug)]
pub enum Footprint {
    Struct(String),
    Field(String),
    Type(String),
}

#[cfg(feature = "footprints")]
impl Footprint {
    pub fn note_struct<S: Into<String>>(string: S) -> Self {
        Self::Struct(string.into())
    }

    pub fn note_field<S: Into<String>>(string: S) -> Self {
        Self::Field(string.into())
    }

    pub fn note_type<S: Into<String>>(string: S) -> Self {
        Self::Type(string.into())
    }
}

pub struct TransportProcessorContext {
    #[cfg(feature = "footprints")]
    footprints: Vec<Footprint>,
    data_map: crate::prelude::TypeMap,
}

impl Default for TransportProcessorContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TransportProcessorContext {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "footprints")]
            footprints: Vec::new(),
            data_map: crate::prelude::TypeMap::new(),
        }
    }

    #[cfg(feature = "footprints")]
    pub fn mark(&mut self, footprint: Footprint) {
        self.footprints.push(footprint)
    }

    #[cfg(feature = "footprints")]
    pub fn footprints(&self) -> &Vec<Footprint> {
        &self.footprints
    }

    pub async fn read_next_var_int<R: AsyncRead + Unpin>(&mut self, read: &mut R) -> Result<i32> {
        crate::extension::read_var_int(self, read).await
    }

    pub fn retrieve_data<T: crate::prelude::Key>(&self) -> Option<&T::Value> {
        self.data_map.get::<T>()
    }

    pub fn retrieve_data_mut<T: crate::prelude::Key>(&mut self) -> Option<&mut T::Value> {
        self.data_map.get_mut::<T>()
    }
}

pub trait DraxTransport {
    fn write_to_transport<W: Write>(
        &self,
        context: &mut TransportProcessorContext,
        writer: &mut W,
    ) -> Result<()>;

    fn read_from_transport<R: Read>(
        context: &mut TransportProcessorContext,
        read: &mut R,
    ) -> Result<Self>
    where
        Self: Sized;

    fn precondition_size(&self, context: &mut TransportProcessorContext) -> Result<usize>;
}
