# Формат `.spx`

`.spx` — это ZIP-архив с бинарными данными проекта, ассетами и историей.

## Версия

Актуальная версия формата хранится в `project.bin` и совпадает с `ARCHIVE_VERSION`.

## Структура архива

```
project.bin                  # ProjectArchiveHeader (MessagePack)
ui.bin                       # UI данные проекта (MessagePack)
blueprints/index.bin         # Индекс блюпринтов (MessagePack)
blueprints/pages/<uuid>.bin  # Блюпринт страницы (MessagePack)
blueprints/server.bin        # Серверный блюпринт (MessagePack)
assets/...                   # Изображения и шрифты
history/timeline.bin         # История изменений (MessagePack)
meta/icon.png                # Иконка проекта
```

## Внутренние бинарные файлы

- `project.bin` (`ProjectArchiveHeader`)
  - `version` — версия архива
  - `manifest` — метаданные проекта
  - `workspace_data` — данные рабочего пространства
  - `icon_path` — путь к иконке в архиве

- `ui.bin`
  - Данные UI-слоя (страницы, элементы, стили, комментарии).

- `blueprints/index.bin` (`BlueprintIndex`)
  - `pages` — список блюпринтов страниц с путями внутри архива
  - `server` — путь к серверному блюпринту (опционально)

## Ассеты

- Изображения: `assets/images/<uuid>.<ext>`
- Шрифты: `assets/fonts/<name>.<ext>`

При удалении объекта или комментария, ссылки на ассеты очищаются, а неиспользуемые файлы удаляются.

## История

- `history/timeline.bin` содержит последние 100 изменений.
- Запись истории обновляется отдельно от основного сохранения проекта.

## Иконка

`meta/icon.png` используется для отображения иконки `.spx` в системе.

