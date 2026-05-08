pub mod blueprint;
pub mod element;

pub use blueprint::{
    add_catalog_node, apply_blueprint_command, bind_catalog_event_node_to_element,
    bind_catalog_event_node_to_element_type, blueprint_node_visual_size, connect_exec_nodes_at,
    connect_nodes_at, delete_node, descriptor_allows_document, duplicate_node, move_node,
    ActionOutcome, BlueprintCommand, ProjectActionError, SourcePinKind,
};
pub use element::{
    apply_geometry_snapshot_recursive, build_page_root, clamp_to_parent_bounds, normalize_rotation,
    page_size, rotate_vector, rotated_bounding_box, set_element_geometry,
    transform_descendant_geometry_with_parent, CanvasElementData, GeometrySnapshot,
};
