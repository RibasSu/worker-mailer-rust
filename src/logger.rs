//! Simple logger with log levels (mirror of TS logger).

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    #[default]
    Info = 1,
    Debug = 0,
    Warn = 2,
    Error = 3,
    None = 4,
}

pub struct Logger {
    level: LogLevel,
    prefix: String,
}

impl Logger {
    pub fn new(level: LogLevel, prefix: impl Into<String>) -> Self {
        Self {
            level,
            prefix: prefix.into(),
        }
    }

    pub fn debug(&self, message: &str) {
        if self.level <= LogLevel::Debug {
            worker::console_log!("{}", format!("{}{}", self.prefix, message));
        }
    }

    pub fn info(&self, message: &str) {
        if self.level <= LogLevel::Info {
            worker::console_log!("{}", format!("{}{}", self.prefix, message));
        }
    }

    pub fn warn(&self, message: &str) {
        if self.level <= LogLevel::Warn {
            worker::console_warn!("{}", format!("{}{}", self.prefix, message));
        }
    }

    pub fn error(&self, message: &str) {
        if self.level <= LogLevel::Error {
            worker::console_error!("{}", format!("{}{}", self.prefix, message));
        }
    }
}
