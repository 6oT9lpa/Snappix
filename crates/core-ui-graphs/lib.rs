//! Core UI graphs for Snappix - Canvas and Slint code generation.
//!
//! This crate provides:
//! - UI element types and layout styles
//! - Canvas state management (selection, history, zoom)
//! - Slint code generation from UI trees
//! - Project data structures

pub mod element;
pub mod layout;
pub mod project;

// Re-exports for convenience
pub use element::{ElementKind, UiElement};
pub use layout::{LayoutStyles, SizeValue};
pub use project::{
    Asset, FormFactor, MetadataProject, Os, Page, Platform, ProjectData, ProjectError,
    ProjectManifest, ProjectMode,
};
