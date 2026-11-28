use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    System,
}

#[derive(Debug, Clone)]
pub struct DiagnosticsLine {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
}

impl DiagnosticsLine {
    pub fn new(level: LogLevel, source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            source: source.into(),
            message: message.into(),
        }
    }

    pub fn format_line(&self) -> String {
        let lvl = match self.level {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERR ",
            LogLevel::System => "SYS ",
        };
        format!(
            "[{}] [{}] [{}] {}",
            self.timestamp.format("%H:%M:%S"),
            lvl,
            self.source,
            self.message
        )
    }
}

#[derive(Debug, Clone)]
pub struct CertPinPrompt {
    pub profile_id: Uuid,
    pub profile_name: String,
    pub server: String,
    pub fingerprint: String,
}
