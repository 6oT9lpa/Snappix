# Project Core

`crates/project-core` is the domain API used by the Snappix UI adapter. It owns
the project editing behavior that used to live in the application layer.

## Public Modules

```text
blueprint.rs        Blueprint graph commands and hit-testing helpers
clipboard.rs        copy/paste operations for Canvas elements
element.rs          CanvasElementData, templates, geometry, layout helpers
history.rs          ProjectSnapshot and HistoryManager
project.rs          Project facade
project_manager.rs  in-memory current-project manager
selection.rs        selection and geometry helper functions
```

## Main Facade: `Project`

`Project` is the aggregate object for an open Snappix project. It wraps the
serialized `ProjectFile` and provides high-level operations for the UI adapter.

Main responsibility groups:

- project metadata and paths;
- page operations;
- open document and active document state;
- Canvas elements and comments;
- managed layout;
- Blueprint documents and active Blueprint graph operations;
- compile, save and load;
- binary snapshots for history.

## Page And Document API

Relevant methods:

- `add_page`
- `remove_page`
- `rename_page`
- `set_active_page`
- `open_document`
- `close_document`
- `select_open_document`
- `page_document_ref`
- `page_blueprint_document_ref`
- `server_blueprint_document_ref`

The application treats visual pages and Blueprint documents as editor documents.
This lets the workspace keep several tabs open: page view, page Blueprint and
server Blueprint.

## Canvas Element API

Relevant methods:

- `add_element_to_active_page_with_parent`
- `update_element_geometry_on_active_page`
- `update_element_text_on_active_page`
- `update_element_style_on_active_page`
- `update_element_container_settings_on_active_page`
- `update_element_parent_on_active_page`
- `group_elements_on_active_page`
- `ungroup_element_on_active_page`
- `remove_element_on_active_page`
- `get_element_on_active_page`
- `active_page_elements`

`CanvasElementData` is the domain representation used by the editor. It carries
id, type, name, geometry, rotation and JSON-like properties. Conversion to/from
`core-ui-graphs::UiElement` is implemented in `element.rs`.

## Layout And Geometry

The core contains pure helpers for:

- rotation normalization;
- rotated bounding boxes;
- clamping children into parent bounds;
- transforming descendants when a parent is resized or rotated;
- applying geometry snapshots recursively;
- computing selection bounds and centers.

These helpers are tested independently, which is important because geometry bugs
are hard to diagnose from the UI alone.

## Comments

Comments are stored on the active page and support:

- content updates;
- position updates;
- size updates;
- title/body font size updates;
- image attachment and clearing;
- deletion.

Comments are part of the project model, so they survive save/load and history
snapshots.

## History

History is implemented through `ProjectSnapshot` and `HistoryManager`.

Flow:

1. Capture snapshot before mutation.
2. Apply domain operation.
3. Capture snapshot after mutation.
4. Record a timeline entry if the state changed.

Snapshots are serialized MessagePack bytes of the project state. Undo and redo
restore the project from those bytes and then force UI synchronization.

## Clipboard

The clipboard module copies selected Canvas elements from the project model and
pastes them into the current project at a target position. It keeps the domain
operation outside the UI adapter and avoids duplicating element traversal logic
in callbacks.

## Blueprint Wrappers

`Project` exposes active Blueprint operations:

- add catalog node;
- create node from a dragged wire;
- move, duplicate and delete nodes;
- connect nodes by drop position;
- bind catalog event nodes to Canvas elements;
- edit input pin values;
- create, duplicate, rename, type and delete local variables;
- compile all Blueprint documents.

These methods call `core-blueprint` and `project-core::blueprint` helpers rather
than implementing graph rules in `apps/`.

## Save And Load

`Project::save` delegates to `project-manager::operations::save_project_with_assets`.
`Project::load` loads a `.spx`, extracts assets into a temporary root and
returns a fully initialized `Project`.

The UI should use `Project` methods rather than calling storage directly.
