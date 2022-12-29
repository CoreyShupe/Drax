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

///// NBT is a tree data structure used and defined in Minecraft's protocol. This is extended to this
///// crate to allow for easy low-level serialization and deserialization of NBT data. This entire
///// module can be omitted by disabling the `nbt` feature.
// pub mod nbt;

/// This module contains all the types and traits necessary for building out a transport layer.
/// Provides a method of directly interacting with the transport layer. A soft-wrapper will be
/// available during serialization and deserialization to account for common types.
pub mod transport;

// stub types
/// VarInt is a variable-sized integer type used for serializing and deserializing data.
///
/// This type is used as a marker for the drax_derive macro, which generates code for serializing and
/// deserializing data using a variable-length encoding. When serializing a value of this type, the
/// drax_derive macro will write the value using a variable-length encoding that uses fewer bytes to
/// represent smaller values and more bytes to represent larger values. When deserializing a value of
/// this type, the drax_derive macro will read the value using the same variable-length encoding.
pub type VarInt = i32;
/// VarLong is a variable-sized long type used for serializing and deserializing data.
///
/// This type is used as a marker for the drax_derive macro, which generates code for serializing and
/// deserializing data using a variable-length encoding. When serializing a value of this type, the
/// drax_derive macro will write the value using a variable-length encoding that uses fewer bytes to
/// represent smaller values and more bytes to represent larger values. When deserializing a value of
/// this type, the drax_derive macro will read the value using the same variable-length encoding.
pub type VarLong = i64;
/// SizedVec describes a list of items with a pre-determined size. When this is used in the
/// drax_derive macro it will read a `VarInt` from the source and then read that many items from the
/// source. This type is used as a marker for the drax_derive macro.
pub type SizedVec<T> = Vec<T>;
/// Maybe describes an optional value. When this is used in the drax_derive macro it will read a
/// boolean from the source. If this value is true it will read in a new value; otherwise it will
/// default itself to `None`. This type is used as a marker for the drax_derive macro.
pub type Maybe<T> = Option<T>;

pub use transport::{
    error::{ErrorType, TransportError, TransportErrorContext},
    Result,
};
