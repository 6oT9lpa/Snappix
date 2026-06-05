# Snappix Technical Documentation

This directory contains the current technical documentation for Snappix. The
documentation is written for developers who need to understand, extend, test or
debug the application.

## Reading Order

1. [Architecture](ARCHITECTURE.md) - high-level system structure and crate
   responsibilities.
2. [Project Core](PROJECT_CORE.md) - the domain API used by the UI adapter.
3. [Canvas Editor](CANVAS_EDITOR.md) - visual editor behavior, drag-and-drop,
   layout and selection.
4. [Blueprint System](BLUEPRINT.md) - graph model, node catalog, validation and
   code generation.
5. [SPX Format](FORMAT_SPX.md) - project archive structure and storage flow.
6. [Logging](LOGGING.md) - runtime diagnostics and log file behavior.
7. [Security](SECURITY.md) - local data safety and archive protection.
8. [Testing](TESTING.md) - current test coverage and expected test strategy.
9. [Development Guide](DEVELOPMENT.md) - commands and contribution rules.

## Core Design Principle

Snappix is split into a UI adapter and a domain core.

`apps/` handles Slint UI, callbacks, hotkeys, menus, user interaction and
model synchronization.

`crates/` contains the application logic:

- project state and operations;
- UI element/domain models;
- Blueprint graph semantics;
- `.spx` storage;
- serialization;
- logging;
- validation and compilation.

When adding a new feature, prefer implementing the real behavior in `crates/`
and calling it from `apps/`. If a large amount of business logic appears inside
`apps/src/editor_runtime/callbacks.rs` or a `.slint` file, the design should be
reviewed.

## Generated And External Materials

The repository may contain diploma-related assets and tools. They are not part
of the runtime application. Keep product documentation in `docs/` and keep
academic/reporting artifacts separate.
