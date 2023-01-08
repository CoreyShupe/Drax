/// Utility for managing the transport layer with `AsyncRead` and `AsyncWrite` types.
pub mod buffer;
/// Encryption and decryption wrappers over `AsyncRead` and `AsyncWrite` types.
#[cfg(feature = "encryption")]
pub mod encryption;
/// Defines a packet struct protocol for reading and writing packets of a generic structure.
pub mod packet;

/// A result type to capture the transport error type.
pub type Result<T> = std::result::Result<T, error::TransportError>;

/// A module defining all the error types for the transport layer.
pub mod error {
    use std::fmt::{Display, Formatter};

    /// The error type for the transport layer.
    #[derive(Debug)]
    pub struct TransportError {
        /// The context around the error.
        pub context: TransportErrorContext,
        /// The cause of the error.
        pub error_type: ErrorType,
    }

    impl TransportError {
        /// Creates a new error with the given error type.
        /// Defaults to use `TransportErrorContext::Unknown` for the context.
        ///
        /// # Parameters
        /// - `error_type`: The type of the error.
        pub fn error(error_type: ErrorType) -> Self {
            Self {
                context: TransportErrorContext::Unknown,
                error_type,
            }
        }

        /// Creates a new error with the given context and error type.
        ///
        /// # Parameters
        /// * `context` - The context of the error.
        /// * `error_type` - The type of the error.
        pub fn with_context(context: TransportErrorContext, error_type: ErrorType) -> Self {
            Self {
                context,
                error_type,
            }
        }
    }

    /// The type of the error.
    #[derive(Debug)]
    pub enum ErrorType {
        /// The error is caused by something generic.
        Generic,
        /// The error is caused by an EOF.
        EOF,
        /// The error is caused by an unknown io error.
        IoError(std::io::Error),
        /// The error is caused by an unknown try from int error.
        TryFromIntError(std::num::TryFromIntError),
        /// The error is caused by an unknown from utf8 error.
        FromUtf8Error(std::string::FromUtf8Error),
        /// The error is caused by an unknown from str::utf8 error.
        Utf8Error(std::str::Utf8Error),
        /// The error is caused by an unknown serde json error.
        #[cfg(feature = "serde")]
        SerdeJsonError(serde_json::Error),
        /// Cesu 8 Decoding Error during NBT parsing.
        #[cfg(feature = "nbt")]
        Cesu8DecodingError(cesu8::Cesu8DecodingError),
        /// The error is caused by an unknown uuid error.
        UuidError(uuid::Error),
    }

    impl std::error::Error for TransportError {}

    impl Display for TransportError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "Transport Error: (Context: {}) ", self.context)?;
            match &self.error_type {
                ErrorType::Generic => write!(f, "Generic Error"),
                ErrorType::EOF => write!(f, "EOF"),
                ErrorType::IoError(err) => write!(f, "IoError {err}"),
                ErrorType::TryFromIntError(err) => write!(f, "TryFromIntError {err}"),
                ErrorType::FromUtf8Error(err) => write!(f, "FromUtf8Error {err}"),
                ErrorType::Utf8Error(err) => write!(f, "Utf8Error {err}"),
                #[cfg(feature = "serde")]
                ErrorType::SerdeJsonError(err) => write!(f, "SerdeJsonError {err}"),
                #[cfg(feature = "nbt")]
                ErrorType::Cesu8DecodingError(err) => write!(f, "Cesu8DecodingError {}", err),
                ErrorType::UuidError(err) => write!(f, "UuidError {err}"),
            }
        }
    }

    /// The context of the error.
    #[derive(Debug)]
    pub enum TransportErrorContext {
        /// The error is caused by something unknown.
        Unknown,
        /// The error is caused by a yeet.
        Yeeted,
        /// The error is explainable by the given string.
        Explainable(String),
    }

    impl From<&str> for TransportErrorContext {
        fn from(str: &str) -> Self {
            Self::Explainable(str.to_string())
        }
    }

    impl From<&String> for TransportErrorContext {
        fn from(str: &String) -> Self {
            Self::Explainable(str.to_string())
        }
    }

    impl From<String> for TransportErrorContext {
        fn from(str: String) -> Self {
            Self::Explainable(str)
        }
    }

    impl Display for TransportErrorContext {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                TransportErrorContext::Unknown => write!(f, "Unknown"),
                TransportErrorContext::Yeeted => write!(f, "Yeeted"),
                TransportErrorContext::Explainable(reason) => write!(f, "`{reason}`"),
            }
        }
    }

    // from binds

    impl From<std::io::Error> for ErrorType {
        fn from(value: std::io::Error) -> Self {
            Self::IoError(value)
        }
    }

    impl From<std::num::TryFromIntError> for ErrorType {
        fn from(value: std::num::TryFromIntError) -> Self {
            Self::TryFromIntError(value)
        }
    }

    impl From<std::string::FromUtf8Error> for ErrorType {
        fn from(value: std::string::FromUtf8Error) -> Self {
            Self::FromUtf8Error(value)
        }
    }

    impl From<std::str::Utf8Error> for ErrorType {
        fn from(value: std::str::Utf8Error) -> Self {
            Self::Utf8Error(value)
        }
    }

    #[cfg(feature = "serde")]
    impl From<serde_json::Error> for ErrorType {
        fn from(value: serde_json::Error) -> Self {
            Self::SerdeJsonError(value)
        }
    }

    #[cfg(feature = "nbt")]
    impl From<cesu8::Cesu8DecodingError> for ErrorType {
        fn from(value: cesu8::Cesu8DecodingError) -> Self {
            ErrorType::Cesu8DecodingError(value)
        }
    }

    impl From<uuid::Error> for ErrorType {
        fn from(value: uuid::Error) -> Self {
            Self::UuidError(value)
        }
    }

    impl<T> From<T> for TransportError
    where
        T: Into<ErrorType>,
    {
        fn from(value: T) -> Self {
            Self {
                context: TransportErrorContext::Yeeted,
                error_type: value.into(),
            }
        }
    }

    // throw macros

    /// Creates a transport error using the given parameters.
    #[macro_export]
    macro_rules! err {
        () => {
            $crate::prelude::TransportError::error($crate::ErrorType::Generic)
        };
        ($error_type:expr) => {
            $crate::prelude::TransportError::error(($error_type).into())
        };
        ($context:expr, $error_type:expr) => {
            $crate::prelude::TransportError::with_context(($context).into(), ($error_type).into())
        };
    }

    /// Creates a generic transport error with the given explanation as context.
    #[macro_export]
    macro_rules! err_explain {
        ($context:expr) => {
            $crate::prelude::TransportError::with_context(
                ($context).into(),
                $crate::prelude::ErrorType::Generic,
            )
        };
    }

    /// Throws a transport error using the given parameters.
    #[macro_export]
    macro_rules! throw {
        () => {
            return Err($crate::err!())
        };
        ($error_type:expr) => {
            return Err($crate::err!($error_type))
        };
        ($context:expr, $error_type:expr) => {
            return Err($crate::err!($context, $error_type))
        };
    }

    /// Throws a generic transport error with the given explanation as context.
    #[macro_export]
    macro_rules! throw_explain {
        ($context:expr) => {
            return Err($crate::err_explain!($context))
        };
    }
}
