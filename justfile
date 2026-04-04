set shell := ["cmd", "/c"]

# Показать доступные команды
default:
    @just --list

# Сборка проекта
build:
    cargo build

# Сборка проекта в релизном режиме
build-release:
    cargo build --release

# Запуск тестов
test:
    cargo test

# Запуск тестов с покрытием
test-coverage:
    cargo tarpaulin --out Html

# Проверка кода на ошибки
check:
    cargo check

# Форматирование кода
fmt:
    cargo fmt

# Линтинг кода
clippy:
    cargo clippy

# Удаление сгенерированных файлов
clean:
    cargo clean

# Запуск конкретного крейта
run-crate crate:
    cargo run -p {{crate}}

# Проверка конкретного крейта
check-crate crate:
    cargo check -p {{crate}}

# Тестирование конкретного крейта
test-crate crate:
    cargo test -p {{crate}}

# Обновление зависимостей
update:
    cargo update

# Аудит зависимостей
audit:
    cargo audit

# Генерация документации
doc:
    cargo doc --open

# Запуск приложения
run:
    cargo run

# Инициализация окружения разработчика
setup:
    cargo install cargo-tarpaulin
    cargo install cargo-audit
    cargo install cargo-watch
    pip install pre-commit

# Установка pre-commit хуков
install-hooks:
    pre-commit install

# Запуск pre-commit хуков на все файлы
pre-commit:
    pre-commit run --all-files

# Запуск полного CI процесса
ci:
    just check
    just test
    just clippy
    just fmt
    just audit

# Отслеживание изменений и сборка
watch-test:
    cargo watch -x test

# Отслеживание изменений и проверка кода
watch-check:
    cargo watch -x check