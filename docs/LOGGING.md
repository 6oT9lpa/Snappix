# Logging

Snappix uses a shared formatted logger implemented in `crates/shared/logging.rs`.

## Default Log File

On Windows the default path is:

```text
%APPDATA%\snappix\logs\snappix.log
```

The path is normalized so the application writes to the roaming AppData area.

## Log Format

Entries are formatted as:

```text
YYYY-MM-DD HH:MM:SS MSK | LEVEL    | target | message
```

Example:

```text
2026-05-09 01:41:23 MSK | INFO     | app | Application started
2026-05-09 01:42:10 MSK | INFO     | project_manager.storage | Saving archive: name='Demo', path='C:\Projects\Demo.spx'
```

Milliseconds are intentionally omitted to keep logs readable for user-facing
diagnostics.

## Logger API

Use the target-based logger for new code:

```rust
shared::logger("project_manager.storage").info(format_args!(
    "Saving project: name='{}', path='{}'",
    project_name,
    path.display()
));
```

Or macros:

```rust
shared::log_info!(
    "project_manager.storage",
    "Archive saved: pages={}, blueprints={}, assets={}",
    pages,
    blueprints,
    assets
);
```

Legacy `log` and `log_fields` APIs still exist for older application events.

## Levels

- `TRACE`
- `DEBUG`
- `INFO`
- `WARNING`
- `ERROR`

Default minimum level is `DEBUG`.

## Rotation

`LoggerConfig` supports:

- enable/disable logger;
- console output;
- file output;
- memory output;
- max file size;
- backup count.

When the log file exceeds the configured size, it is rotated to numbered backup
files.

## What Should Be Logged

Log important application and core stages:

- application start/close;
- user login;
- project open/save/load;
- archive entry read/write;
- autosave skip/save decisions;
- explicit `Ctrl+S`;
- shutdown/close save attempts;
- Blueprint compile pipeline stages;
- validation failures;
- file operations that may fail;
- user actions that mutate project state.

Do not log:

- passwords or secrets;
- full binary payloads;
- huge serialized structures;
- every pointer move during drag preview.

For interactive actions, log commit points rather than every UI tick.
