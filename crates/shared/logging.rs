use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use std::collections::VecDeque;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
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
            Self::Warn => "WARN",
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
    pub category: LogCategory,
    pub message: LogMessage,
    pub fields: Vec<LogField>,
}

#[derive(Debug, Clone)]
pub struct LoggerConfig {
    pub min_level: LogLevel,
    pub stderr_enabled: bool,
    pub file_enabled: bool,
    pub file_path: PathBuf,
    pub memory_enabled: bool,
    pub memory_capacity: usize,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Info,
            stderr_enabled: true,
            file_enabled: true,
            file_path: default_log_file_path(),
            memory_enabled: true,
            memory_capacity: 512,
        }
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

pub fn log(level: LogLevel, category: LogCategory, message: LogMessage) {
    record(LogEntry {
        timestamp_ms: timestamp_ms(),
        level,
        category,
        message,
        fields: Vec::new(),
    });
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
    record(LogEntry {
        timestamp_ms: timestamp_ms(),
        level,
        category,
        message,
        fields,
    });
}

pub fn recent_log_entries() -> Vec<LogEntry> {
    with_logger(|state| state.entries.iter().cloned().collect())
}

pub fn clear_log_entries() {
    with_logger(|state| state.entries.clear());
}

fn record(entry: LogEntry) {
    with_logger(|state| {
        if entry.level < state.config.min_level {
            return;
        }

        let formatted = format_entry(&entry);
        if state.config.stderr_enabled {
            eprintln!("{formatted}");
        }

        if state.config.file_enabled {
            write_log_line(
                &state.config.file_path,
                &formatted,
                state.config.stderr_enabled,
            );
        }

        if state.config.memory_enabled {
            state.entries.push_back(entry);
            trim_entries(state);
        }
    });
}

fn write_log_line(path: &PathBuf, line: &str, report_to_stderr: bool) {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            if report_to_stderr {
                eprintln!("Failed to create log directory {}: {err}", parent.display());
            }
            return;
        }
    }

    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut file) => {
            if let Err(err) = writeln!(file, "{line}") {
                if report_to_stderr {
                    eprintln!("Failed to write log file {}: {err}", path.display());
                }
            }
        }
        Err(err) => {
            if report_to_stderr {
                eprintln!("Failed to open log file {}: {err}", path.display());
            }
        }
    }
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
    let fields = if entry.fields.is_empty() {
        String::new()
    } else {
        let fields = entry
            .fields
            .iter()
            .map(|field| format!("{}={}", field.key, escape_field_value(&field.value)))
            .collect::<Vec<_>>()
            .join(" ");
        format!(" {fields}")
    };
    format!(
        "[{}] {} {} {} - {}{}",
        format_timestamp_msk(entry.timestamp_ms),
        entry.level,
        entry.category,
        entry.message.code(),
        entry.message.template(),
        fields
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
    msk.format("%Y-%m-%d %H:%M:%S%.3f MSK").to_string()
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

    #[test]
    fn logger_keeps_recent_entries_in_memory() {
        let _guard = test_lock();
        configure_logger(LoggerConfig {
            min_level: LogLevel::Trace,
            stderr_enabled: false,
            file_enabled: false,
            file_path: default_log_file_path(),
            memory_enabled: true,
            memory_capacity: 4,
        });
        clear_log_entries();

        log(LogLevel::Info, LogCategory::App, LogMessage::AppStarted);

        let entries = recent_log_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, LogMessage::AppStarted);
    }

    #[test]
    fn logger_filters_below_min_level() {
        let _guard = test_lock();
        configure_logger(LoggerConfig {
            min_level: LogLevel::Warn,
            stderr_enabled: false,
            file_enabled: false,
            file_path: default_log_file_path(),
            memory_enabled: true,
            memory_capacity: 4,
        });
        clear_log_entries();

        log(LogLevel::Info, LogCategory::App, LogMessage::AppStarted);
        log(
            LogLevel::Error,
            LogCategory::App,
            LogMessage::AppCloseFailed,
        );

        let entries = recent_log_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Error);
    }

    #[test]
    fn logger_trims_to_capacity() {
        let _guard = test_lock();
        configure_logger(LoggerConfig {
            min_level: LogLevel::Trace,
            stderr_enabled: false,
            file_enabled: false,
            file_path: default_log_file_path(),
            memory_enabled: true,
            memory_capacity: 2,
        });
        clear_log_entries();

        log(LogLevel::Info, LogCategory::App, LogMessage::AppStarted);
        log(
            LogLevel::Info,
            LogCategory::Project,
            LogMessage::ProjectOpened,
        );
        log(
            LogLevel::Info,
            LogCategory::Project,
            LogMessage::ProjectSaved,
        );

        let entries = recent_log_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, LogMessage::ProjectOpened);
        assert_eq!(entries[1].message, LogMessage::ProjectSaved);
    }

    #[test]
    fn logger_records_structured_fields() {
        let _guard = test_lock();
        configure_logger(LoggerConfig {
            min_level: LogLevel::Trace,
            stderr_enabled: false,
            file_enabled: false,
            file_path: default_log_file_path(),
            memory_enabled: true,
            memory_capacity: 4,
        });
        clear_log_entries();

        log_fields(
            LogLevel::Info,
            LogCategory::Project,
            LogMessage::ProjectOpened,
            [("path", "demo.spx")],
        );

        let entries = recent_log_entries();
        assert_eq!(entries[0].fields[0], LogField::new("path", "demo.spx"));
    }

    #[test]
    fn logger_appends_entries_to_file() {
        let _guard = test_lock();
        let path = std::env::temp_dir().join(format!("snappix-logger-test-{}.log", timestamp_ms()));
        let _ = std::fs::remove_file(&path);
        configure_logger(LoggerConfig {
            min_level: LogLevel::Trace,
            stderr_enabled: false,
            file_enabled: true,
            file_path: path.clone(),
            memory_enabled: false,
            memory_capacity: 0,
        });
        clear_log_entries();

        log_fields(
            LogLevel::Info,
            LogCategory::Project,
            LogMessage::ProjectOpened,
            [("path", "demo project.spx")],
        );

        let content = std::fs::read_to_string(&path).expect("read log file");
        assert!(content.contains("project.opened"));
        assert!(content.contains("MSK] INFO project"));
        assert!(content.contains("path=\"demo project.spx\""));
        let _ = std::fs::remove_file(path);
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
        assert_eq!(format_timestamp_msk(0), "1970-01-01 03:00:00.000 MSK");
    }
}
