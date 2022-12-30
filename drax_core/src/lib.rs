//! # Drax
//!
//! Drax is a library which supports framed packet reading and writing.
//! Drax itself is not an implementation of any protocol but instead a framework to build protocols
//! on top of. <br />
//! <br />
//! This framework should be able to provide all the tooling necessary for building an entire server
//! stack. The framework will attempt to keep most types generic and provide no default
//! implementations other than the low-level t1 layer. <br />
//! <br />
//! Drax will attempt to provide a low-overhead SDK for building out serialization and
//! deserialization for packets. These packets can be composed from bytes directly to reduce the
//! amount of allocations and copying required. While the bytes are drained from the source they're
//! used to build out the correlating types. <br />

/// NBT is a tree data structure used and defined in Minecraft's protocol. This is extended to this
/// crate to allow for easy low-level serialization and deserialization of NBT data. This entire
/// module can be omitted by disabling the `nbt` feature.
#[cfg(feature = "nbt")]
pub mod nbt;

/// This module contains all the types and traits necessary for building out a transport layer.
/// Provides a method of directly interacting with the transport layer. A soft-wrapper will be
/// available during serialization and deserialization to account for common types.
pub mod transport;

pub use transport::{
    error::{ErrorType, TransportError, TransportErrorContext},
    Result,
};
