#[cfg(feature = "encryption")]
mod encryption;

use std::fmt::{Display, Formatter};
use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Debug)]
pub enum Error {
    Unknown(Option<String>),
    TokioError(tokio::io::Error),
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
        match self {
            Error::Unknown(potential_reason) => match potential_reason {
                None => write!(f, "Unknown error"),
                Some(reason) => write!(f, "Caught reason: {}", reason),
            },
            Error::TokioError(err) => write!(f, "Tokio error: {}", err),
        }
    }
}

impl std::error::Error for Error {}

impl From<tokio::io::Error> for Error {
    fn from(tokio_error: tokio::io::Error) -> Self {
        Self::TokioError(tokio_error)
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

    pub async fn read_next_var_int<R: AsyncRead + Unpin>(&mut self, read: &mut R) -> Result<i32> {
        crate::extension::read_var_int(self, read).await
    }
}

pub trait Processor {}

pub struct TransportProcessor {
    context: TransportProcessorContext,
    processors: Vec<Box<dyn Processor>>,
}
