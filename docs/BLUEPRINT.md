# Blueprint System

The Blueprint system is the visual logic subsystem of Snappix. It is implemented
mostly in `crates/core-blueprint` and exposed to the UI through `project-core`.

## Core Files

```text
crates/core-blueprint/model.rs     graph, node, pin and document model
crates/core-blueprint/catalog.rs   built-in node descriptors
crates/core-blueprint/api.rs       UI/page/server API descriptors
crates/core-blueprint/validate.rs  diagnostics and validation rules
crates/core-blueprint/lowering.rs  graph to IR transformation
crates/core-blueprint/codegen.rs   Rust code generation
crates/core-blueprint/compile.rs   generated workspace write + cargo check
crates/project-core/blueprint.rs   graph commands used by Project
```

## Document Types

Snappix uses two Blueprint document types:

- `PageBlueprint` - logic scoped to a UI page. It may reference UI elements and
  their events/actions.
- `ServerBlueprint` - project/server logic that must not directly reference UI
  elements.

This split is enforced by validation and by node descriptor contexts.

## Graph Model

Main model types:

- `BlueprintDocument`;
- `BlueprintGraph`;
- `BlueprintNode`;
- `BlueprintPin`;
- `BlueprintLink`;
- `BlueprintLocalVariable`;
- `BlueprintFunctionSignature`;
- `BlueprintExport`.

Nodes are connected through pins. Links are explicit graph edges between output
pins and input pins.

## Exec Pins And Data Pins

Blueprint pins are divided into two categories.

Exec pins define execution order. If an action node is not reachable through an
exec chain from an entrypoint, it will not execute even if its data pins have
values.

Data pins transfer typed values. Current data types include:

- `Bool`;
- `Int`;
- `Float`;
- `String`;
- `Color`;
- `Object`;
- `Array`;
- `UiElementRef`;
- `PageRef`;
- `ApiRef`;
- `Any`.

Data output pins may connect to multiple input pins. Data input pins accept only
one active link; a new compatible link replaces the old input link.

## Node Catalog

Built-in nodes are declared in `catalog.rs` through descriptors. The descriptor
defines:

- id;
- title;
- category;
- allowed context;
- tags;
- input/output pins.

Current node categories include:

- Events;
- Flow;
- Values;
- Variables;
- Math;
- Compare;
- Convert;
- UI actions;
- Functions;
- Server/API-related nodes.

Math and compare nodes are pure data nodes: they do not have exec pins. UI
actions and state-changing operations use exec pins because execution order
matters.

## Input Pin Values

Unconnected data input pins can store local values:

- bool defaults to `true`/`false`;
- int defaults to `0`;
- float defaults to `0.0`;
- string defaults to an empty string;
- object/page/element values can reference project entities;
- arrays are not edited as raw text values and should be manipulated through
  dedicated nodes/modes.

The UI renders these editors in `BlueprintNodeCard.slint`, while the value is
stored in the Blueprint model.

## Variables

Blueprint local variables support:

- creation;
- duplication;
- deletion;
- renaming;
- type changes;
- item type for collections;
- object binding for object-like variables;
- getter and setter node creation.

Deleting a variable removes related variable nodes and incompatible links.

## Validation

Validation runs before lowering/compilation and reports diagnostics with
severity, code, message, document id, graph id, node id and optional pin id.

Validation checks include:

- unknown functional/catalog descriptors;
- wrong node context for page/server Blueprint;
- missing UI element references;
- invalid UI event source;
- incompatible pin links;
- invalid static display/collection modes;
- empty string variable used as collection mode;
- exec-aware setter checks for values assigned earlier in the execution chain.

Warnings are shown near nodes in the UI. The full diagnostic message should be
available through a tooltip.

## Compilation Pipeline

`Project::compile_blueprints` calls `core_blueprint::compile_project`.

Pipeline:

1. `validate_project` checks all documents.
2. `lower_project` transforms graphs into an intermediate representation.
3. `generate_project` writes a Rust workspace representation.
4. `cargo check --message-format=json` validates generated code.
5. compiler diagnostics are mapped back to Blueprint nodes through source maps.

The result is `BlueprintCompilationResult`, which contains success status,
output directory, generated files, source map and diagnostics.

## UI Integration

The Blueprint UI lives in:

- `BlueprintEditorScene.slint`;
- `BlueprintCanvas.slint`;
- `BlueprintNodeCard.slint`;
- `BlueprintSidebar.slint`.

The UI supports:

- node creation from context menu;
- node creation from dropped wire;
- node movement;
- node duplication and deletion;
- multi-delete;
- variable context actions;
- source binding for event nodes;
- input value editing;
- object/page/element pickers;
- diagnostics display.

## Geometry Contract

Node size and pin offsets are used in both UI rendering and core hit testing.
When changing `BlueprintNodeCard.slint`, verify corresponding helpers in
`project-core/blueprint.rs`, especially:

- `blueprint_node_visual_size`;
- node hit testing;
- target input pin selection by drop position.
