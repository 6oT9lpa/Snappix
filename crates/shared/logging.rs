use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use std::collections::VecDeque;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::log_messages::LogMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARNING",
            Self::Error => "ERROR",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogCategory {
    App,
    Project,
    Blueprint,
    Asset,
    Config,
    Runtime,
}

impl fmt::Display for LogCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::App => "app",
            Self::Project => "project",
            Self::Blueprint => "blueprint",
            Self::Asset => "asset",
            Self::Config => "config",
            Self::Runtime => "runtime",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogField {
    pub key: String,
    pub value: String,
}

impl LogField {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub timestamp_ms: u128,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct LoggerConfig {
    pub enabled: bool,
    pub min_level: LogLevel,
    pub console_enabled: bool,
    pub file_enabled: bool,
    pub file_path: PathBuf,
    pub max_bytes: u64,
    pub backup_count: usize,
    pub memory_enabled: bool,
    pub memory_capacity: usize,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_level: LogLevel::Debug,
            console_enabled: true,
            file_enabled: true,
            file_path: default_log_file_path(),
            max_bytes: 1_048_576,
            backup_count: 5,
            memory_enabled: true,
            memory_capacity: 512,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Logger {
    target: String,
}

impl Logger {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
        }
    }

    pub fn trace(&self, message: fmt::Arguments<'_>) {
        self.record(LogLevel::Trace, message);
    }

    pub fn debug(&self, message: fmt::Arguments<'_>) {
        self.record(LogLevel::Debug, message);
    }

    pub fn info(&self, message: fmt::Arguments<'_>) {
        self.record(LogLevel::Info, message);
    }

    pub fn warn(&self, message: fmt::Arguments<'_>) {
        self.record(LogLevel::Warn, message);
    }

    pub fn error(&self, message: fmt::Arguments<'_>) {
        self.record(LogLevel::Error, message);
    }

    fn record(&self, level: LogLevel, message: fmt::Arguments<'_>) {
        record(LogEntry {
            timestamp_ms: timestamp_ms(),
            level,
            target: self.target.clone(),
            message: message.to_string(),
        });
    }
}

#[derive(Debug)]
struct LoggerState {
    config: LoggerConfig,
    entries: VecDeque<LogEntry>,
}

impl Default for LoggerState {
    fn default() -> Self {
        Self {
            config: LoggerConfig::default(),
            entries: VecDeque::new(),
        }
    }
}

static LOGGER: OnceLock<Mutex<LoggerState>> = OnceLock::new();

pub fn configure_logger(config: LoggerConfig) {
    with_logger(|state| {
        state.config = config;
        trim_entries(state);
    });
}

pub fn default_log_file_path() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let base = PathBuf::from(appdata);

        // The launcher may provide either %APPDATA% itself or its parent. Normalize
        // to the requested roaming path: %APPDATA%\snappix\logs\snappix.log.
        let roaming_base = if base
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("Roaming"))
            .unwrap_or(false)
        {
            base
        } else {
            base.join("Roaming")
        };
        return roaming_base
            .join("snappix")
            .join("logs")
            .join("snappix.log");
    }

    std::env::temp_dir()
        .join("snappix")
        .join("logs")
        .join("snappix.log")
}

pub fn logger(target: impl Into<String>) -> Logger {
    Logger::new(target)
}

pub fn log(level: LogLevel, category: LogCategory, message: LogMessage) {
    logger(category.to_string()).record(level, format_args!("{}", message.template()));
}

pub fn log_fields<I, K, V>(level: LogLevel, category: LogCategory, message: LogMessage, fields: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    let fields = fields
        .into_iter()
        .map(|(key, value)| LogField::new(key, value))
        .collect::<Vec<_>>();
    let suffix = format_field_suffix(&fields);
    logger(category.to_string()).record(level, format_args!("{}{}", message.template(), suffix));
}

pub fn recent_log_entries() -> Vec<LogEntry> {
    with_logger(|state| state.entries.iter().cloned().collect())
}

pub fn clear_log_entries() {
    with_logger(|state| state.entries.clear());
}

fn record(entry: LogEntry) {
    with_logger(|state| {
        if !state.config.enabled || entry.level < state.config.min_level {
            return;
        }

        let formatted = format_entry(&entry);
        if state.config.console_enabled {
            eprintln!("{formatted}");
        }

        if state.config.file_enabled {
            write_log_line(&state.config, &formatted);
        }

        if state.config.memory_enabled {
            state.entries.push_back(entry);
            trim_entries(state);
        }
    });
}

fn write_log_line(config: &LoggerConfig, line: &str) {
    if let Some(parent) = config.file_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            if config.console_enabled {
                eprintln!("Failed to create log directory {}: {err}", parent.display());
            }
            return;
        }
    }

    rotate_log_file_if_needed(config, line.len() as u64 + 1);

    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.file_path)
    {
        Ok(mut file) => {
            if let Err(err) = writeln!(file, "{line}") {
                if config.console_enabled {
                    eprintln!(
                        "Failed to write log file {}: {err}",
                        config.file_path.display()
                    );
                }
            }
        }
        Err(err) => {
            if config.console_enabled {
                eprintln!(
                    "Failed to open log file {}: {err}",
                    config.file_path.display()
                );
            }
        }
    }
}

fn rotate_log_file_if_needed(config: &LoggerConfig, incoming_bytes: u64) {
    if config.max_bytes == 0 || !config.file_path.exists() {
        return;
    }

    let Ok(metadata) = fs::metadata(&config.file_path) else {
        return;
    };
    if metadata.len().saturating_add(incoming_bytes) <= config.max_bytes {
        return;
    }

    if config.backup_count == 0 {
        let _ = fs::remove_file(&config.file_path);
        return;
    }

    // Rotate in descending order so snappix.log.1 is not overwritten before it
    // has been moved to snappix.log.2.
    let oldest = rotated_log_path(&config.file_path, config.backup_count);
    let _ = fs::remove_file(oldest);

    for index in (1..config.backup_count).rev() {
        let from = rotated_log_path(&config.file_path, index);
        if from.exists() {
            let to = rotated_log_path(&config.file_path, index + 1);
            let _ = fs::rename(from, to);
        }
    }

    let first_backup = rotated_log_path(&config.file_path, 1);
    let _ = fs::rename(&config.file_path, first_backup);
}

fn rotated_log_path(path: &Path, index: usize) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snappix.log");
    path.with_file_name(format!("{file_name}.{index}"))
}

fn with_logger<T>(f: impl FnOnce(&mut LoggerState) -> T) -> T {
    let logger = LOGGER.get_or_init(|| Mutex::new(LoggerState::default()));
    let mut guard = logger
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn trim_entries(state: &mut LoggerState) {
    while state.entries.len() > state.config.memory_capacity {
        state.entries.pop_front();
    }
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn format_entry(entry: &LogEntry) -> String {
    format!(
        "{} | {:<8} | {} | {}",
        format_timestamp_msk(entry.timestamp_ms),
        entry.level,
        entry.target,
        entry.message
    )
}

fn format_timestamp_msk(timestamp_ms: u128) -> String {
    let seconds = (timestamp_ms / 1000).min(i64::MAX as u128) as i64;
    let nanos = ((timestamp_ms % 1000) * 1_000_000) as u32;
    let utc =
        DateTime::<Utc>::from_timestamp(seconds, nanos).unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
    let msk = FixedOffset::east_opt(3 * 60 * 60)
        .expect("MSK fixed offset")
        .from_utc_datetime(&utc.naive_utc());
    msk.format("%Y-%m-%d %H:%M:%S MSK").to_string()
}

fn format_field_suffix(fields: &[LogField]) -> String {
    if fields.is_empty() {
        return String::new();
    }

    let fields = fields
        .iter()
        .map(|field| format!("{}={}", field.key, escape_field_value(&field.value)))
        .collect::<Vec<_>>()
        .join(" ");
    format!(" {fields}")
}

fn escape_field_value(value: &str) -> String {
    if value
        .chars()
        .all(|ch| !ch.is_whitespace() && ch != '"' && ch != '\\')
    {
        return value.to_string();
    }

    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[macro_export]
macro_rules! log_trace {
    ($target:expr, $($arg:tt)+) => {
        $crate::logger($target).trace(format_args!($($arg)+))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($target:expr, $($arg:tt)+) => {
        $crate::logger($target).debug(format_args!($($arg)+))
    };
}

#[macro_export]
macro_rules! log_info {
    ($target:expr, $($arg:tt)+) => {
        $crate::logger($target).info(format_args!($($arg)+))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($target:expr, $($arg:tt)+) => {
        $crate::logger($target).warn(format_args!($($arg)+))
    };
}

#[macro_export]
macro_rules! log_error {
    ($target:expr, $($arg:tt)+) => {
        $crate::logger($target).error(format_args!($($arg)+))
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn test_config() -> LoggerConfig {
        LoggerConfig {
            enabled: true,
            min_level: LogLevel::Trace,
            console_enabled: false,
            file_enabled: false,
            file_path: default_log_file_path(),
            max_bytes: 1_048_576,
            backup_count: 5,
            memory_enabled: true,
            memory_capacity: 4,
        }
    }

    #[test]
    fn logger_keeps_recent_entries_in_memory() {
        let _guard = test_lock();
        configure_logger(test_config());
        clear_log_entries();

        logger("app").info(format_args!("Application started"));

        let entries = recent_log_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].target, "app");
        assert_eq!(entries[0].message, "Application started");
    }

    #[test]
    fn logger_filters_below_min_level() {
        let _guard = test_lock();
        configure_logger(LoggerConfig {
            min_level: LogLevel::Warn,
            ..test_config()
        });
        clear_log_entries();

        logger("app").info(format_args!("Application started"));
        logger("app").error(format_args!("Application close failed"));

        let entries = recent_log_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Error);
    }

    #[test]
    fn logger_can_be_disabled() {
        let _guard = test_lock();
        configure_logger(LoggerConfig {
            enabled: false,
            ..test_config()
        });
        clear_log_entries();

        logger("app").error(format_args!("Application close failed"));

        assert!(recent_log_entries().is_empty());
    }

    #[test]
    fn logger_trims_to_capacity() {
        let _guard = test_lock();
        configure_logger(LoggerConfig {
            memory_capacity: 2,
            ..test_config()
        });
        clear_log_entries();

        logger("app").info(format_args!("one"));
        logger("project").info(format_args!("two"));
        logger("project").info(format_args!("three"));

        let entries = recent_log_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "two");
        assert_eq!(entries[1].message, "three");
    }

    #[test]
    fn logger_records_formatted_messages() {
        let _guard = test_lock();
        configure_logger(test_config());
        clear_log_entries();

        logger("framework.elements.base_element").info(format_args!(
            "[{}] Searching element: name='{}', locator={}, condition='{}'",
            "button", "Submit", "#submit", "visible"
        ));

        let entries = recent_log_entries();
        assert_eq!(entries[0].target, "framework.elements.base_element");
        assert_eq!(
            entries[0].message,
            "[button] Searching element: name='Submit', locator=#submit, condition='visible'"
        );
    }

    #[test]
    fn legacy_log_fields_records_readable_message() {
        let _guard = test_lock();
        configure_logger(test_config());
        clear_log_entries();

        log_fields(
            LogLevel::Info,
            LogCategory::Project,
            LogMessage::ProjectOpened,
            [("path", "demo project.spx")],
        );

        let entries = recent_log_entries();
        assert_eq!(entries[0].target, "project");
        assert_eq!(
            entries[0].message,
            "Project opened path=\"demo project.spx\""
        );
    }

    #[test]
    fn logger_appends_entries_to_file() {
        let _guard = test_lock();
        let path = std::env::temp_dir().join(format!("snappix-logger-test-{}.log", timestamp_ms()));
        let _ = std::fs::remove_file(&path);
        configure_logger(LoggerConfig {
            file_enabled: true,
            file_path: path.clone(),
            memory_enabled: false,
            ..test_config()
        });
        clear_log_entries();

        logger("project").info(format_args!("Project opened path='{}'", "demo project.spx"));

        let content = std::fs::read_to_string(&path).expect("read log file");
        assert!(content.contains("MSK | INFO"));
        assert!(content.contains(" | project | "));
        assert!(content.contains("Project opened path='demo project.spx'"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn logger_rotates_file_when_it_exceeds_limit() {
        let _guard = test_lock();
        let path =
            std::env::temp_dir().join(format!("snappix-logger-rotate-{}.log", timestamp_ms()));
        let backup = rotated_log_path(&path, 1);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);
        std::fs::write(&path, "existing log line that is already too large\n")
            .expect("write old log");
        configure_logger(LoggerConfig {
            file_enabled: true,
            file_path: path.clone(),
            max_bytes: 8,
            backup_count: 1,
            memory_enabled: false,
            ..test_config()
        });
        clear_log_entries();

        logger("project").info(format_args!("new line"));

        assert!(path.exists());
        assert!(backup.exists());
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(backup);
    }

    #[test]
    fn logger_default_path_points_to_snappix_log_file() {
        let path = default_log_file_path();
        assert!(path.ends_with("snappix.log"));
        assert!(path
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("snappix"));
    }

    #[test]
    fn logger_formats_timestamp_as_msk_datetime() {
        assert_eq!(format_timestamp_msk(0), "1970-01-01 03:00:00 MSK");
    }
}
