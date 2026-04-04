//! Project management for Snappix.
//!
//! This crate provides:
//! - Project creation, saving, and loading
//! - Binary format storage using MessagePack
//! - Project structure management

pub mod format;
pub mod operations;
pub mod storage;

// Re-exports for convenience
pub use format::{EditorDocumentRef, ProjectFile, WorkspaceData};
pub use operations::{create_project, load_project, save_project};
pub use storage::{ProjectStorage, StorageError};
