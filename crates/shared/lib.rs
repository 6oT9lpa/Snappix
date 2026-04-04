//! Shared types and utilities for Snappix.
//!
//! This crate provides common types, error handling, and serialization
//! utilities used across all other crates in the workspace.

pub mod error;
pub mod id;
pub mod position;
pub mod serialization;

// Re-exports for convenience
pub use error::{Result, SnappixError};
pub use id::generate_id;
pub use position::{Position, Rect};
pub use serialization::{from_msgpack, to_msgpack};
