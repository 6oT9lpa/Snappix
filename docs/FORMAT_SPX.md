# SPX Project Format

`.spx` is the portable project file format used by Snappix. It is implemented
as a ZIP archive containing MessagePack-serialized project data and binary
assets.

## Why ZIP + MessagePack

ZIP provides:

- a single portable project artifact;
- separated internal entries;
- asset storage;
- compression;
- inspectable archive structure.

MessagePack provides:

- compact binary serialization;
- faster load/save than text-heavy formats for large projects;
- stable Rust serialization through `serde`.

## Archive Entries

Current entries written by `ProjectStorage`:

```text
project.bin                 ProjectArchiveHeader
ui.bin                      UiData
blueprints/index.bin        BlueprintIndex
blueprints/pages/<uuid>.bin Page Blueprint document per page
blueprints/server.bin       Server Blueprint document
history/timeline.bin        History bytes
meta/icon.png               Project icon
assets/...                  Project assets
```

## Header

`project.bin` stores `ProjectArchiveHeader`:

- archive version;
- project manifest;
- workspace data;
- optional icon path.

The header lets the application read important project metadata without treating
the whole archive as one monolithic blob.

## UI Data

`ui.bin` stores:

- pages;
- page element trees;
- comments;
- assets metadata.

Page elements are restored into `CanvasElementData` and then synchronized into
Slint models.

## Blueprint Data

Blueprint documents are split:

- page Blueprints are stored independently under `blueprints/pages/`;
- server Blueprint is stored as `blueprints/server.bin`;
- `blueprints/index.bin` maps page ids to archive paths.

This split makes migration and partial handling easier than a single logic blob.

## Save Flow

```text
Project::save
-> project-manager::operations::save_project_with_assets
-> ProjectStorage::save
-> ProjectStorage::save_archive
-> write project.bin
-> write ui.bin
-> write Blueprint index/documents
-> write history
-> write icon
-> copy assets
-> finish ZIP archive
```

The storage layer logs each important step: archive path, entry names, byte
counts, page counts, Blueprint counts and asset counts.

## Load Flow

```text
Project::load
-> project-manager::operations::load_project
-> ProjectStorage::load
-> load_archive
-> read header
-> read UI data
-> read Blueprint index and documents
-> read server document
-> extract assets
-> initialize Project facade
```

If archive loading fails, the loader attempts legacy MessagePack and JSON
formats for compatibility.

## History

History is stored in `history/timeline.bin` when history persistence is
requested. Autosaves may be lighter than close-time saves, depending on the
caller.

## Assets

Assets are stored under `assets/...`. Extraction validates internal archive
paths before writing to disk.

See [Security](SECURITY.md) for path traversal protection.

## Compatibility Rules

When changing the format:

- update the archive version if the change is incompatible;
- add migration or fallback logic when possible;
- keep old projects loadable when the cost is reasonable;
- add tests for archive read/write and asset path safety.
