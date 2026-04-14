//! # dangyang
//!
//! A library for parsing YANG `typedef` statements and generating Rust types
//! from them. Designed for use inside a `build.rs` script.
//!
//! ## Quick start — build.rs
//!
//! ```rust,no_run
//! use dang_yang::{parse_file, TypeRegistry, CodeGenerator};
//!
//! fn main() {
//!     // Tell the code generator which YANG types map to which Rust types.
//!     let mut registry = TypeRegistry::new();
//!     registry.register("ip-address", "std::net::IpAddr");
//!     registry.register("port-number", "u16");
//!
//!     // Parse all typedef statements from the YANG source file.
//!     let typedefs = parse_file("src/model.yang").unwrap();
//!
//!     // Generate Rust source code.
//!     let code = CodeGenerator::new(&registry).generate(&typedefs);
//!
//!     let out = std::env::var("OUT_DIR").unwrap();
//!     std::fs::write(format!("{out}/yang_types.rs"), code).unwrap();
//!
//!     println!("cargo:rerun-if-changed=src/model.yang");
//! }
//! ```
//!
//! Then in your main crate:
//!
//! ```rust,ignore
//! include!(concat!(env!("OUT_DIR"), "/yang_types.rs"));
//! ```
//!
//! ## Custom type mappings
//!
//! By default, YANG built-in types are mapped to their natural Rust
//! equivalents (`string` → `String`, `uint32` → `u32`, etc.).  Use
//! [`TypeRegistry::register`] to override any derived type:
//!
//! ```rust
//! use dang_yang::TypeRegistry;
//!
//! let mut registry = TypeRegistry::new();
//!
//! // Plain name
//! registry.register("ip-address", "std::net::IpAddr");
//!
//! // Module-prefixed name (both forms are checked during resolution)
//! registry.register("ietf-inet-types:ipv6-address", "std::net::Ipv6Addr");
//!
//! // A type from your own crate
//! registry.register("my-custom-id", "crate::MyId");
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

pub mod ast;
pub mod codegen;
pub mod error;
mod lexer;
#[cfg(feature = "std")]
pub mod library;
mod parser;
pub mod registry;
pub mod value;

#[cfg(test)]
mod tests;

pub use ast::{BitDef, EnumVariant, Restriction, Status, TypeStmt, TypedefNode};
pub use codegen::CodeGenerator;
pub use error::ParseError;
#[cfg(feature = "std")]
pub use library::{LibraryError, YangLibrary, YangObject};
pub use registry::TypeRegistry;
pub use value::YangValue;

/// Parse all `typedef` statements from a YANG source string.
///
/// Top-level `module`/`submodule` wrappers are handled automatically;
/// all other statements are skipped.
pub fn parse_str(source: &str) -> Result<Vec<TypedefNode>, ParseError> {
    parser::parse_typedefs(source)
}

/// Parse all `typedef` statements from a YANG file on disk.
#[cfg(feature = "std")]
pub fn parse_file(path: impl AsRef<std::path::Path>) -> Result<Vec<TypedefNode>, ParseError> {
    let source = std::fs::read_to_string(path)?;
    parse_str(&source)
}
