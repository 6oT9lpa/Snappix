//! Error types for Snappix.

use thiserror::Error;

/// Main error type for Snappix operations.
#[derive(Error, Debug)]
pub enum SnappixError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("MessagePack serialization error: {0}")]
    MsgPack(#[from] rmp_serde::encode::Error),

    #[error("MessagePack deserialization error: {0}")]
    MsgPackDecode(#[from] rmp_serde::decode::Error),

    #[error("Project error: {0}")]
    Project(String),

    #[error("Graph error: {0}")]
    Graph(String),

    #[error("Code generation error: {0}")]
    CodeGen(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

/// Result type alias for Snappix operations.
pub type Result<T> = std::result::Result<T, SnappixError>;
