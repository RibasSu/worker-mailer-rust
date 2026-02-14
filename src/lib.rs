//! worker-mailer — Send emails via SMTP from Cloudflare Workers (Rust).
//!
//! Port of the TypeScript [@ribassu/worker-mailer](https://github.com/RibasSu/worker-mailer) for use in Cloudflare Workers with Rust.

pub mod email;
pub mod errors;
pub mod logger;
pub mod mailer;
pub mod queue;
pub mod utils;

// Re-exports
pub use email::{Attachment, DsnNotify, DsnOverride, DsnRet, Email, EmailBuildError, EmailOptions, Recipient, User};
pub use errors::{
    InvalidContentError, InvalidEmailError, SmtpAuthError, SmtpConnectionError, SmtpRecipientError,
    SmtpTimeoutError, WorkerMailerError,
};
pub use logger::{LogLevel, Logger};
pub use mailer::{
    AuthType, Credentials, DsnNotify as DsnNotifyOpt, DsnOptions, DsnRet as DsnRetOpt,
    WorkerMailer, WorkerMailerHooks, WorkerMailerOptions,
};
pub use queue::{enqueue_email, enqueue_emails, process_batch, QueueEmailMessage, QueueProcessResult};
pub use utils::{decode, encode_header, encode_quoted_printable, is_valid_email, validate_emails};
