# Snappix

Snappix is a desktop visual application builder written in Rust with a Slint UI.
It combines a Canvas editor for interface design, a Blueprint editor for
application logic, and a portable `.spx` project archive format.

The product is designed as a local-first no-code/low-code environment: project
data, assets, UI structure, Blueprint graphs, workspace state, history snapshots
and logs are processed on the user's machine without sending project data to
external services.

## What Snappix Provides

- Visual Canvas editor for building application pages with drag-and-drop.
- Component tree, properties panel, selection, grouping, comments and hotkeys.
- Managed layout containers: Stack, Flex and Grid.
- Blueprint editor with typed exec/data pins, event nodes, flow nodes,
  variables, math, compare, conversion and UI action nodes.
- Page Blueprint and Server Blueprint document types.
- Blueprint validation before compilation.
- Rust code generation and `cargo check` verification for generated Blueprint
  projects.
- `.spx` archive format based on ZIP + MessagePack.
- Project history, undo/redo, autosave and explicit save through `Ctrl+S`.
- Structured logging with MSK timestamps and log rotation.

## Repository Structure

```text
apps/                    Slint desktop application and UI adapter layer
crates/shared/           common errors, ids, serialization and logging
crates/core-ui-graphs/   base UI graph/layout data structures
crates/core-blueprint/   Blueprint model, catalog, validation, lowering, codegen
crates/project-core/     project domain API: pages, elements, assets, history
crates/project-manager/  .spx format, storage and filesystem operations
docs/                    technical documentation
```

`apps/` should stay focused on UI, user interaction and mapping domain data into
Slint models. The core product logic belongs in `crates/`.

## Quick Start

Install stable Rust and run:

```bash
cargo run -p slint-rust
```

Useful commands:

```bash
cargo check --workspace
cargo test --workspace
cargo check -p slint-rust
cargo test -p core-blueprint -p project-core -p shared
```

The repository also includes a `justfile` with common commands, although some
comments in that file may still contain legacy encoding artifacts.

## Documentation

- [Documentation Index](docs/README.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Project Core](docs/PROJECT_CORE.md)
- [Canvas Editor](docs/CANVAS_EDITOR.md)
- [Blueprint System](docs/BLUEPRINT.md)
- [SPX Format](docs/FORMAT_SPX.md)
- [Logging](docs/LOGGING.md)
- [Security](docs/SECURITY.md)
- [Testing](docs/TESTING.md)
- [Development Guide](docs/DEVELOPMENT.md)

## Current Engineering Status

The current architecture separates the UI adapter from the domain core:

- `project-core` owns the `Project` facade, page operations, element operations,
  comments, history, clipboard, selection helpers, Blueprint wrappers and
  save/load integration.
- `core-blueprint` owns Blueprint graph semantics, catalog descriptors,
  diagnostics, lowering and code generation.
- `project-manager` owns archive storage and filesystem-facing operations.
- `apps` wires Slint callbacks to domain operations and synchronizes Slint
  models after state changes.

At the time this documentation was rewritten, the workspace contains 90 Rust
tests across the core crates and application runtime modules.

## License

The repository contains a `LICENSE` file. Confirm the final distribution model
before public release.
