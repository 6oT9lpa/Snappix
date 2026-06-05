# Testing

Snappix currently has a Rust-first automated test strategy. The most important
business logic lives in `crates/`, so most tests target core crates rather than
the Slint UI directly.

## Current Test Count

At the time this documentation was rewritten, the workspace contains 90 Rust
tests:

```text
core-blueprint/catalog.rs: 1
core-blueprint/compile.rs: 5
core-blueprint/validate.rs: 11
core-ui-graphs/element.rs: 2
core-ui-graphs/layout.rs: 2
project-core/blueprint.rs: 10
project-core/element.rs: 22
project-core/project.rs: 14
project-manager/operations/mod.rs: 10
project-manager/storage/mod.rs: 2
shared/logging.rs: 10
shared/serialization.rs: 1
```

## Recommended Commands

```bash
cargo test --workspace
cargo test -p core-blueprint
cargo test -p project-core
cargo test -p shared
cargo check -p slint-rust
```

For quick regression after Blueprint/UI work:

```bash
cargo test -p core-blueprint -p project-core -p shared
cargo check -p slint-rust
```

## What Is Covered

### `core-blueprint`

Tests cover:

- catalog descriptors;
- event entrypoints;
- page/server context validation;
- incompatible pin links;
- invalid input modes;
- compile pipeline scenarios;
- source diagnostics.

### `project-core`

Tests cover:

- element templates;
- geometry and rotation;
- parent-bound clamping;
- descendant transforms;
- Blueprint graph commands;
- multiple data output links;
- object/int compatibility for data links;
- active Blueprint diagnostics;
- variable deletion and node cleanup;
- project save/load round trips;
- simple Blueprint compilation through the project facade.

### `project-manager`

Tests cover:

- project operations;
- safe archive asset paths;
- path traversal rejection;
- history/storage helpers.

### `shared`

Tests cover:

- MessagePack roundtrip;
- logger memory storage;
- level filtering;
- disabled logger behavior;
- formatted messages;
- file appending;
- rotation;
- MSK timestamp formatting.

## Manual UI Checks

Because Slint UI behavior is not fully automated yet, run manual checks after
large UI changes:

- create/open project;
- add page;
- drag components onto Canvas;
- move, resize, rotate and group elements;
- edit text/style/layout properties;
- use `Ctrl+S`;
- create Blueprint nodes;
- connect exec and data pins;
- edit unconnected input data pins;
- create node from wire drop;
- duplicate/delete variables;
- verify validation badges and tooltips;
- save and reopen `.spx`.

## Future AQA Direction

The next testing milestone should add UI-oriented automation:

- screenshot regression for key editor states;
- scripted Canvas drag/drop scenarios;
- Blueprint wire interaction scenarios;
- tooltip positioning checks;
- hotkey checks;
- save/load smoke tests with generated projects.

The goal is not to test Slint internals. The goal is to verify visible user
outcomes and domain state transitions.
