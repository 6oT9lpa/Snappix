# Snappix

Snappix — это визуальный редактор проектов с поддержкой UI, логики и ассетов в едином архиве `.spx`.

## Быстрый старт

1. Установите Rust (stable) и `cargo`.
2. Соберите и запустите приложение:

```bash
cargo build
cargo run -p slint-rust
```

## Что внутри

- `apps/` — приложение и UI на Slint.
- `crates/` — ядро, хранение проектов и компиляция блюпринтов.
- `docs/` — архитектура, формат `.spx`, гайд по разработке.

## Формат `.spx`

`.spx` — ZIP-архив с бинарными файлами и ресурсами проекта. Детали формата: [docs/FORMAT_SPX.md](E:/snappix/docs/FORMAT_SPX.md)

## Документация

- Архитектура: [docs/ARCHITECTURE.md](E:/snappix/docs/ARCHITECTURE.md)
- Разработка: [docs/DEVELOPMENT.md](E:/snappix/docs/DEVELOPMENT.md)

## Поддерживаемые платформы

- Windows
- Linux (файловые ассоциации `.spx` через `xdg-mime`)

