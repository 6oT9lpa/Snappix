pub mod blueprint;
pub mod clipboard;
pub mod element;
pub mod history;
pub mod project;
pub mod project_manager;
pub mod selection;

pub use blueprint::{
    add_catalog_node, apply_blueprint_command, bind_catalog_event_node_to_element,
    bind_catalog_event_node_to_element_type, blueprint_node_visual_size, connect_exec_nodes_at,
    connect_nodes_at, delete_node, descriptor_allows_document, duplicate_node, move_node,
    prune_incompatible_links, ActionOutcome, BlueprintCommand, ProjectActionError, SourcePinKind,
};
pub use element::{
    apply_geometry_snapshot_recursive, build_page_root, clamp_to_parent_bounds, normalize_rotation,
    page_size, rotate_vector, rotated_bounding_box, set_element_geometry,
    transform_descendant_geometry_with_parent, CanvasElementData, GeometrySnapshot,
};
pub use history::{HistoryActionKind, HistoryManager, PreviewSelection, ProjectSnapshot};
pub use project::{DevMode, EditorDocumentKind, PageSize, Platform, Project};
pub use project_manager::ProjectManager;
pub use selection::{
    is_finite_geometry, is_finite_style_values, normalize_rotation_degrees, rects_intersect,
    rotated_selection_bounds, selected_and_descendant_ids, selected_root_ids, selection_center,
};
