//! Error types for WorkerMailer (mirror of TS module).

use thiserror::Error;

/// Base error for WorkerMailer.
#[derive(Error, Debug)]
#[error("{message}")]
pub struct WorkerMailerError {
    pub message: String,
    pub code: &'static str,
}

/// Invalid email address(es).
#[derive(Error, Debug)]
#[error("{message}")]
pub struct InvalidEmailError {
    pub message: String,
    pub invalid_emails: Vec<String>,
}

impl InvalidEmailError {
    pub const CODE: &'static str = "INVALID_EMAIL";
    pub fn new(message: impl Into<String>, invalid_emails: Vec<String>) -> Self {
        Self {
            message: message.into(),
            invalid_emails,
        }
    }
}

/// SMTP authentication failed.
#[derive(Error, Debug)]
#[error("{0}")]
pub struct SmtpAuthError(pub String);

impl SmtpAuthError {
    pub const CODE: &'static str = "AUTH_FAILED";
}

/// SMTP connection failed.
#[derive(Error, Debug)]
#[error("{0}")]
pub struct SmtpConnectionError(pub String);

impl SmtpConnectionError {
    pub const CODE: &'static str = "CONNECTION_FAILED";
}

/// Recipient rejected by SMTP server.
#[derive(Error, Debug)]
#[error("{message}")]
pub struct SmtpRecipientError {
    pub message: String,
    pub recipient: String,
}

impl SmtpRecipientError {
    pub const CODE: &'static str = "RECIPIENT_REJECTED";
    pub fn new(message: impl Into<String>, recipient: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            recipient: recipient.into(),
        }
    }
}

/// SMTP operation timeout.
#[derive(Error, Debug)]
#[error("{0}")]
pub struct SmtpTimeoutError(pub String);

impl SmtpTimeoutError {
    pub const CODE: &'static str = "TIMEOUT";
}

/// Invalid email content (e.g. missing text and html).
#[derive(Error, Debug)]
#[error("{0}")]
pub struct InvalidContentError(pub String);

impl InvalidContentError {
    pub const CODE: &'static str = "INVALID_CONTENT";
}
