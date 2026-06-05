# Architecture

Snappix is a Rust workspace organized around a strict split between UI and core
logic.

## Workspace Members

```text
apps                  desktop application, Slint UI, UI adapter
crates/shared         common errors, ids, serialization, logging
crates/core-ui-graphs UI graph and layout data types
crates/core-blueprint Blueprint model, validation, lowering, code generation
crates/project-core   domain facade for project editing
crates/project-manager .spx archive format and storage operations
```

## Layer Model

### UI Layer: `apps/`

Responsibilities:

- render the application window and editor scenes;
- receive mouse, keyboard and menu events;
- maintain Slint-specific model structs;
- map user callbacks into domain method calls;
- synchronize domain state back into the UI;
- show validation diagnostics, tooltips and context menus.

Important files:

- `apps/ui/app-window.slint` - main Slint window.
- `apps/ui/views/VisualEditorScene.slint` - Canvas editor scene.
- `apps/ui/views/BlueprintEditorScene.slint` - Blueprint editor scene.
- `apps/ui/editor/Canvas.slint` - visual Canvas surface.
- `apps/ui/editor/BlueprintCanvas.slint` - Blueprint graph surface.
- `apps/ui/editor/BlueprintNodeCard.slint` - Blueprint node rendering.
- `apps/src/editor_runtime/callbacks.rs` - callback registration and UI action
  adapter.
- `apps/src/editor_runtime/sync.rs` - domain-to-Slint synchronization.

The UI layer is not expected to own project rules. It should ask the domain
core to perform operations and then refresh UI models.

### Domain Layer: `crates/project-core`

Responsibilities:

- project lifecycle and active document handling;
- page creation, deletion, renaming and selection;
- Canvas element creation, geometry, style, parent-child structure and layout;
- comments;
- clipboard;
- history and undo/redo snapshots;
- active Blueprint wrappers;
- project save/load facade.

The main type is `project_core::Project`. The UI adapter normally interacts
with the application through this facade.

### Blueprint Layer: `crates/core-blueprint`

Responsibilities:

- typed graph model;
- node catalog descriptors;
- pin types and default input values;
- page/server Blueprint validation;
- graph lowering into an intermediate representation;
- Rust code generation;
- generated workspace verification through `cargo check`;
- diagnostic mapping through source maps.

### Storage Layer: `crates/project-manager`

Responsibilities:

- `.spx` archive read/write;
- MessagePack serialization layout;
- asset extraction;
- history persistence;
- project file relocation/deletion helpers;
- legacy load fallback.

### Shared Layer: `crates/shared`

Responsibilities:

- `SnappixError`;
- UUID helpers;
- MessagePack helpers;
- positions and rectangles;
- structured logger and log macros.

## User Action Flow

Typical flow:

1. A user interacts with a Slint component.
2. The component emits a callback with minimal UI data.
3. `editor_runtime/callbacks.rs` validates the UI input, exits history preview
   mode if needed and calls `Project`.
4. `Project` mutates domain state and returns success/failure.
5. The callback records history when appropriate.
6. The project is saved immediately or through autosave throttling.
7. `editor_runtime/sync.rs` rebuilds Slint models from the domain state.

Example:

```text
Canvas drop -> element-dropped callback
            -> Project::add_element_to_active_page_with_parent
            -> HistoryManager::record_change
            -> save_project_silent / save_project_forced
            -> sync_editor_models
```

## Why This Split Matters

The split keeps the system testable. A project operation can be tested in Rust
without launching Slint, and the UI can stay focused on presentation and
interaction.

The architecture also prevents feature drift. If `apps/` starts doing direct
filesystem operations, Blueprint validation or layout algorithms, that logic
should usually be moved to `project-core`, `core-blueprint` or
`project-manager`.

## Current Risk Areas

- `apps/src/editor_runtime/callbacks.rs` is still large because it wires many UI
  events. Keep future business logic out of it.
- Slint node geometry and `project-core` Blueprint hit testing must stay in
  sync. If node sizes or pin offsets change in UI, update the corresponding
  core helpers and tests.
- `.spx` is a persisted format. Structural changes must include migration or
  explicit compatibility handling.
