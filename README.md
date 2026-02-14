# Worker Mailer (Rust)

[English](./README.md) | [Português](./README_pt-BR.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Worker Mailer is an SMTP client for **Cloudflare Workers** written in **Rust**. It is a port of [@ribassu/worker-mailer](https://github.com/RibasSu/worker-mailer) (TypeScript) and uses [Cloudflare TCP Sockets](https://developers.cloudflare.com/workers/runtime-apis/tcp-sockets/) via the [`worker`](https://docs.rs/worker) crate.

## Features

- 🚀 Built for the Cloudflare Workers runtime (compiles to `wasm32-unknown-unknown`)
- 📧 Send plain text and HTML emails with attachments
- 🖼️ Inline image attachments with Content-ID (CID) support
- 🔒 SMTP authentication: **PLAIN** and **LOGIN** (CRAM-MD5 not implemented in this port)
- ✅ Email address validation (RFC 5322 compliant)
- 🎯 Custom error types for better error handling
- 🪝 Lifecycle hooks for monitoring email operations
- 📅 DSN support (options structs)
- 📬 Cloudflare Queues integration for async email processing

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [API Reference](#api-reference)
- [Inline Images (CID)](#inline-images-cid)
- [Lifecycle Hooks](#lifecycle-hooks)
- [Error Handling](#error-handling)
- [Cloudflare Queues Integration](#cloudflare-queues-integration)
- [Limitations](#limitations)
- [Building for Workers](#building-for-workers)
- [License](#license)

## Installation

Add to your Worker's `Cargo.toml`:

```toml
[dependencies]
worker = { version = "0.7", features = ["queue"] }
worker-mailer = { path = "../worker-mailer-rust" }
# or from crates.io when published:
# worker-mailer = "0.1"
```

## Quick Start

1. Ensure your Worker project targets `wasm32-unknown-unknown` and uses the `worker` crate with the `queue` feature if you need Queues.

2. Connect and send an email:

```rust
use worker_mailer::{
    WorkerMailer, WorkerMailerOptions, Credentials, AuthType,
    EmailOptions, Recipient, LogLevel,
};

#[worker::event(fetch)]
async fn fetch(
    _req: worker::Request,
    _env: worker::Env,
    _ctx: worker::Context,
) -> Result<worker::Response, worker::Error> {
    let options = WorkerMailerOptions {
        host: "smtp.example.com".to_string(),
        port: 587,
        secure: false,
        start_tls: true,
        credentials: Some(Credentials {
            username: "your-smtp-user".to_string(),
            password: "your-smtp-password".to_string(),
        }),
        auth_type: vec![AuthType::Plain],
        log_level: LogLevel::Info,
        ..Default::default()
    };

    let email_options = EmailOptions {
        from: Recipient::Email("from@example.com".to_string()),
        to: vec![Recipient::Email("to@example.com".to_string())],
        subject: "Hello from Worker Mailer".to_string(),
        text: Some("This is a plain text message.".to_string()),
        html: Some("<h1>Hello</h1><p>This is HTML.</p>".to_string()),
        ..Default::default()
    };

    WorkerMailer::send(options, email_options).await?;
    worker::Response::ok("OK")
}
```

3. For production, use secrets (e.g. from `env.secret("SMTP_USER")`) instead of hardcoded credentials.

## API Reference

### WorkerMailer::connect(options)

Creates a new SMTP connection. Returns a `WorkerMailer` that can send multiple emails over the same connection.

```rust
pub struct WorkerMailerOptions {
    pub host: String,
    pub port: u16,
    pub secure: bool,           // Use TLS (default: false)
    pub start_tls: bool,        // Upgrade to TLS if supported (default: true)
    pub credentials: Option<Credentials>,
    pub auth_type: Vec<AuthType>, // e.g. vec![AuthType::Plain]
    pub log_level: LogLevel,
    pub dsn: Option<DsnOptions>,
    pub socket_timeout_ms: u64,
    pub response_timeout_ms: u64,
    pub hooks: WorkerMailerHooks,
}

pub struct Credentials {
    pub username: String,
    pub password: String,
}

pub enum AuthType { Plain, Login, CramMd5 }
```

### mailer.send_one(options)

Sends one email on an existing connection.

```rust
let mut mailer = WorkerMailer::connect(options).await?;
mailer.send_one(email_options).await?;
mailer.close(None).await?;
```

### WorkerMailer::send(options, email_options)

Sends a single email without keeping the connection open (connect, send, close).

```rust
WorkerMailer::send(mailer_options, email_options).await?;
```

### EmailOptions

```rust
pub struct EmailOptions {
    pub from: Recipient,
    pub to: Vec<Recipient>,
    pub reply: Option<Recipient>,
    pub cc: Option<Vec<Recipient>>,
    pub bcc: Option<Vec<Recipient>>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub attachments: Option<Vec<Attachment>>,
    pub dsn_override: Option<DsnOverride>,
}

pub enum Recipient {
    Email(String),
    User(User),
}

pub struct User {
    pub email: String,
    pub name: Option<String>,
}

pub struct Attachment {
    pub filename: String,
    pub content: String,      // Base64-encoded content
    pub mime_type: Option<String>,
    pub cid: Option<String>,  // Content-ID for inline images
    pub inline: Option<bool>,
}
```

## Inline Images (CID)

Embed images in HTML emails using Content-ID (CID):

```rust
use worker_mailer::{WorkerMailer, WorkerMailerOptions, EmailOptions, Recipient, Attachment};

let mailer = WorkerMailer::connect(options).await?;

mailer.send_one(EmailOptions {
    from: Recipient::Email("sender@example.com".to_string()),
    to: vec![Recipient::Email("recipient@example.com".to_string())],
    subject: "Email with embedded image".to_string(),
    html: Some(r#"
        <h1>Hello!</h1>
        <p>Here's our logo:</p>
        <img src="cid:company-logo" alt="Company Logo">
    "#.to_string()),
    attachments: Some(vec![Attachment {
        filename: "logo.png".to_string(),
        content: logo_base64,
        mime_type: Some("image/png".to_string()),
        cid: Some("company-logo".to_string()),
        inline: Some(true),
    }]),
    ..Default::default()
}).await?;
```

## Lifecycle Hooks

Monitor email operations with hooks (set on `WorkerMailerOptions.hooks`):

```rust
use worker_mailer::WorkerMailerHooks;

let options = WorkerMailerOptions {
    hooks: WorkerMailerHooks {
        on_connect: Some(Box::new(|| {
            worker::console_log!("Connected to SMTP server");
        })),
        on_sent: Some(Box::new(|email, response| {
            worker::console_log!(format!("Email sent: {}", response));
        })),
        on_error: Some(Box::new(|email, err| {
            worker::console_error!(format!("Send failed: {}", err));
        })),
        on_close: Some(Box::new(|err| {
            if let Some(e) = err {
                worker::console_error!(format!("Connection closed: {}", e));
            }
        })),
        ..Default::default()
    },
    ..your_options
};
```

## Error Handling

The crate uses `worker::Error` and custom error types for SMTP failures:

```rust
use worker_mailer::{WorkerMailer, WorkerMailerOptions, EmailOptions, Recipient};

match WorkerMailer::send(options, email_options).await {
    Ok(()) => worker::Response::ok("Sent"),
    Err(e) => {
        let msg = e.to_string();
        if msg.contains("AUTH") {
            // Authentication failed
        } else if msg.contains("Invalid email") {
            // InvalidEmailError (when building Email)
        }
        worker::Response::error(msg, 500)
    }
}
```

When building an `Email` with `Email::new(options)`, you can get `EmailBuildError::InvalidContent` (missing text/html) or `EmailBuildError::InvalidEmail` (invalid addresses).

## Cloudflare Queues Integration

For high-volume or async email sending, use Cloudflare Queues.

### Setup

1. Add a Queue producer and consumer in `wrangler.toml`:

```toml
[[queues.producers]]
queue = "email-queue"
binding = "EMAIL_QUEUE"

[[queues.consumers]]
queue = "email-queue"
max_batch_size = 10
max_retries = 3
```

2. Implement the queue handler in your worker:

```rust
use worker_mailer::{process_batch, QueueEmailMessage, MessageExt};

#[worker::event(queue)]
async fn queue(
    batch: worker::MessageBatch<QueueEmailMessage>,
    _env: worker::Env,
    _ctx: worker::Context,
) -> Result<(), worker::Error> {
    let results = process_batch(batch).await;
    for r in &results {
        worker::console_log!(format!("success={} error={:?}", r.success, r.error));
    }
    Ok(())
}
```

3. Enqueue emails from your fetch handler:

```rust
use worker_mailer::{enqueue_email, enqueue_emails, QueueEmailMessage};

// Single email
enqueue_email(&env.queue("EMAIL_QUEUE")?, &QueueEmailMessage {
    mailer_options: options,
    email_options: email_opts,
}).await?;

// Batch
enqueue_emails(&env.queue("EMAIL_QUEUE")?, &[msg1, msg2]).await?;
```

## Limitations

- **Port 25:** Cloudflare Workers cannot make outbound connections on port 25. Use 587 or 465.
- **Auth:** Only PLAIN and LOGIN are implemented. CRAM-MD5 is not (would require HMAC-MD5 in WASM).
- **Connections:** Each Worker instance has limits on concurrent TCP connections. Close connections when done (e.g. `mailer.close(None).await`).

## Building for Workers

```bash
# Using worker-build (recommended)
cargo install worker-build
worker-build

# Or manually
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
```

## License

This project is licensed under the MIT License.
