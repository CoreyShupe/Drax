pub mod encryption;
pub mod pipeline;

pub type Result<T> = std::result::Result<T, error::TransportError>;

pub mod error {
    use std::fmt::{Display, Formatter};

    #[derive(Debug)]
    pub struct TransportError {
        context: TransportErrorContext,
        error_type: ErrorType,
    }

    impl TransportError {
        pub fn error(error_type: ErrorType) -> Self {
            Self {
                context: TransportErrorContext::Unknown,
                error_type,
            }
        }

        pub fn with_context(context: TransportErrorContext, error_type: ErrorType) -> Self {
            Self {
                context,
                error_type,
            }
        }
    }

    #[derive(Debug)]
    pub enum ErrorType {
        EOF,
        IoError(std::io::Error),
        TryFromIntError(std::num::TryFromIntError),
        FromUtf8Error(std::string::FromUtf8Error),
        SerdeJsonError(serde_json::Error),
    }

    impl std::error::Error for TransportError {}

    impl Display for TransportError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "Transport Error: (Context: {}) ", self.context)?;
            match &self.error_type {
                ErrorType::EOF => write!(f, "EOF"),
                ErrorType::IoError(err) => write!(f, "IoError {}", err),
                ErrorType::TryFromIntError(err) => write!(f, "TryFromIntError {}", err),
                ErrorType::FromUtf8Error(err) => write!(f, "FromUtf8Error {}", err),
                ErrorType::SerdeJsonError(err) => write!(f, "SerdeJsonError {}", err),
            }
        }
    }

    #[derive(Debug)]
    pub enum TransportErrorContext {
        Unknown,
        Yeeted,
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
                TransportErrorContext::Explainable(reason) => write!(f, "`{}`", reason),
            }
        }
    }

    // from binds

    impl From<std::io::Error> for TransportError {
        fn from(value: std::io::Error) -> Self {
            Self {
                context: TransportErrorContext::Yeeted,
                error_type: ErrorType::IoError(value),
            }
        }
    }

    impl From<std::io::Error> for ErrorType {
        fn from(value: std::io::Error) -> Self {
            Self::IoError(value)
        }
    }

    impl From<std::num::TryFromIntError> for TransportError {
        fn from(value: std::num::TryFromIntError) -> Self {
            Self {
                context: TransportErrorContext::Yeeted,
                error_type: ErrorType::TryFromIntError(value),
            }
        }
    }

    impl From<std::num::TryFromIntError> for ErrorType {
        fn from(value: std::num::TryFromIntError) -> Self {
            Self::TryFromIntError(value)
        }
    }

    impl From<std::string::FromUtf8Error> for TransportError {
        fn from(value: std::string::FromUtf8Error) -> Self {
            Self {
                context: TransportErrorContext::Yeeted,
                error_type: ErrorType::FromUtf8Error(value),
            }
        }
    }

    impl From<std::string::FromUtf8Error> for ErrorType {
        fn from(value: std::string::FromUtf8Error) -> Self {
            Self::FromUtf8Error(value)
        }
    }

    impl From<serde_json::Error> for TransportError {
        fn from(value: serde_json::Error) -> Self {
            Self {
                context: TransportErrorContext::Yeeted,
                error_type: ErrorType::SerdeJsonError(value),
            }
        }
    }

    impl From<serde_json::Error> for ErrorType {
        fn from(value: serde_json::Error) -> Self {
            Self::SerdeJsonError(value)
        }
    }

    // throw macros

    #[macro_export]
    macro_rules! throw {
        ($error_type:expr) => {
            return $crate::t2::TransportError::error(($error_type).into());
        };
        ($context:expr, $error_type:expr) => {
            return $crate::t2::TransportError::with_context($context, ($error_type).into());
        };
    }
}
