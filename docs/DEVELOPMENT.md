# Разработка

## Требования

- Rust (stable)
- Cargo

## Сборка

```bash
cargo build
```

## Запуск приложения

```bash
cargo run -p slint-rust
```

## Проверки

```bash
cargo check
cargo test
```

## Структура репозитория

- `apps/` — UI и взаимодействие с пользователем.
- `crates/project-manager/` — операции сохранения/загрузки `.spx`.
- `crates/core-blueprint/` — компиляция блюпринтов.

## Работа с `.spx`

Основные операции:

- `project-manager::storage::save` — сохранить проект в `.spx`.
- `project-manager::storage::load` — загрузить проект из `.spx`.
- `project-manager::operations::save_project_history` — сохранить историю.

Подробности структуры архива см. в [docs/FORMAT_SPX.md](E:/snappix/docs/FORMAT_SPX.md).

## Linux: ассоциации `.spx`

При запуске приложения выполняется регистрация `.spx` через `xdg-mime` и `desktop`-файл.
Если ассоциации не применились, проверьте:

- наличие `xdg-mime` в системе
- права на `~/.local/share/`

