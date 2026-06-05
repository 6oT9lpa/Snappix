//! Shared types and utilities for Snappix.
//!
//! This crate provides common types, error handling, and serialization
//! utilities used across all other crates in the workspace.

pub mod error;
pub mod id;
pub mod log_messages;
pub mod logging;
pub mod position;
pub mod serialization;

// Re-exports for convenience
pub use error::{Result, SnappixError};
pub use id::generate_id;
pub use log_messages::LogMessage;
pub use logging::{
    clear_log_entries, configure_logger, default_log_file_path, log, log_fields, logger,
    recent_log_entries, LogCategory, LogEntry, LogField, LogLevel, Logger, LoggerConfig,
};
pub use position::{Position, Rect};
pub use serialization::{from_msgpack, to_msgpack};
