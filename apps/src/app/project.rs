//! Compatibility adapter for the project domain API.
//!
//! The implementation lives in `crates/project-core`; `apps` keeps this module
//! only so existing UI adapter imports can be migrated incrementally.

pub use project_core::{
    CanvasElementData, DevMode, EditorDocumentKind, PageSize, Platform, Project, ProjectManager,
};
