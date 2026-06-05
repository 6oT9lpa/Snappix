# Security And Data Integrity

Snappix is a local-first editor. The main security concerns are project file
integrity, safe filesystem operations and preventing invalid user-created logic
from reaching runtime code.

## Local Data Model

Project data is stored locally in `.spx` files. The application does not need to
send project data to an external service for normal editing, saving or
compilation.

## Archive Path Safety

`.spx` files are ZIP archives. Asset extraction validates archive paths through
`safe_archive_asset_path`.

Rejected path patterns include:

- absolute paths;
- paths containing `..`;
- entries that would write outside the intended asset root.

This protects against ZIP path traversal attacks.

## Filesystem Operations

Filesystem-sensitive operations should live in `project-manager`, not in the UI
layer.

Examples:

- load project;
- save project;
- extract assets;
- delete/relocate project path;
- save/load history.

The UI should call a tested storage/domain API and map errors into UI
notifications.

## Save Safety

Snappix supports:

- explicit save through `Ctrl+S`;
- autosave with throttling;
- forced save on window close/exit flows;
- history serialization when requested.

The goal is to reduce data loss while avoiding heavy disk writes on every UI
event.

## Blueprint Validation As Safety

Blueprint validation prevents incorrect logic from reaching generated code.

Examples:

- server Blueprint cannot directly access UI elements;
- page-only nodes cannot be used in server context;
- catalog nodes must exist;
- pin links must be type-compatible;
- UI event nodes must match source element capabilities;
- invalid collection/display modes become diagnostics;
- missing element references are reported before compilation.

## Logging Safety

Logs should be detailed enough for diagnostics but must not contain secrets or
large payloads. Prefer structured, human-readable messages that identify the
stage and key ids/paths.

## Current Limitations

- `.spx` archives are not encrypted.
- There is no signed archive verification.
- There is no role-based access control because the current application is a
  local desktop editor.

These can be added later if Snappix gains collaboration, cloud sync or shared
project repositories.
