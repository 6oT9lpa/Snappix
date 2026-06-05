# Canvas Editor

The Canvas editor is the visual interface construction surface in Snappix. It is
implemented in Slint but its real operations are backed by `project-core`.

## Main UI Files

- `apps/ui/views/VisualEditorScene.slint`
- `apps/ui/editor/Canvas.slint`
- `apps/ui/editor/ComponentLibrary.slint`
- `apps/ui/editor/ComponentExplorer.slint`
- `apps/ui/editor/PropertiesPanel.slint`
- `apps/ui/editor/PageTabs.slint`
- `apps/src/editor_runtime/callbacks.rs`
- `apps/src/editor_runtime/sync.rs`

## Component Drop Flow

The user drags a component from the component library onto the Canvas.

High-level flow:

```text
ComponentLibrary item drag
-> VisualEditorScene drag state
-> Canvas drop coordinates
-> callbacks.rs element-dropped handler
-> Project::add_element_to_active_page_with_parent
-> history entry
-> save/autosave
-> sync_editor_models
```

The Canvas editor converts screen coordinates into page/model coordinates and
then asks the project core to create the element. If the drop is outside the
page bounds, the operation should be ignored.

## Preview And Commit

Interactive transforms are split into two phases:

- preview: local UI feedback while the mouse is moving;
- commit: one domain mutation when the action finishes.

This prevents excessive history entries, repeated save operations and expensive
model synchronization on every pointer movement.

## Selection

Selection supports:

- single element selection;
- multi-selection;
- marquee selection;
- selection bounds;
- outline navigation;
- selected root filtering so child elements are not processed twice when their
  parent is also selected.

Selection helpers live in `project-core::selection`.

## Geometry Operations

The editor supports:

- move;
- resize;
- rotation;
- horizontal and vertical flip;
- group and ungroup;
- parent change through outline drag/drop;
- image source assignment;
- style and text updates.

Geometry is stored in the domain model. Slint displays the result through
synced `SelectionInfo` and Canvas model data.

## Managed Containers

Snappix supports three managed container modes:

- Stack;
- Flex;
- Grid.

The core validates and normalizes child geometry according to the selected
container mode.

Stack places children sequentially and uses padding, margin, spacing and
alignment.

Flex follows a CSS Flexbox-like model with direction, wrap, justify-content,
align-items, align-content and gap.

Grid follows a grid-like model with columns, rows and gaps.

When a container changes, `relayout_container_on_active_page` recalculates the
affected subtree rather than recomputing the entire scene.

## Properties Panel

The properties panel edits:

- x/y position;
- width/height;
- rotation;
- text and placeholder;
- checked state;
- background, border, text color and font;
- opacity and display mode;
- image source;
- container mode and layout settings.

All changes should map to project-core methods. The panel should not implement
domain-specific validation on its own beyond UI affordances.

## Comments

Canvas comments are page-level objects stored with the project:

- position;
- size;
- title/body;
- font sizes;
- optional image.

They are included in history snapshots and `.spx` save/load.

## Hotkeys

Important hotkeys include:

- `Ctrl+S` - force save;
- undo/redo;
- copy/cut/paste;
- delete;
- group/ungroup;
- hide selection;
- reset view;
- toggle panels/comments.

Hotkey state is configured through `apps/src/editor_runtime/hotkeys.rs` and the
Slint scene bindings.

## Performance Notes

The Canvas avoids unnecessary work by:

- committing transforms only at the end of the operation;
- throttling regular autosave;
- syncing derived Slint models after completed mutations;
- recalculating managed layout only for affected containers/subtrees;
- keeping domain state in Rust rather than inside Slint visual components.
