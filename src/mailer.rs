//! SMTP client for Cloudflare Workers (mirror of TS mailer).

use crate::email::{Email, EmailOptions};
use crate::logger::{LogLevel, Logger};
use crate::utils::{decode, encode};
use worker::ConnectionBuilder;
use worker::Socket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

/// Auth methods supported by the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    Plain,
    Login,
    CramMd5,
}

/// SMTP credentials.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Hooks for mailer events (not serialized for queue).
#[derive(Default)]
pub struct WorkerMailerHooks {
    pub on_connect: Option<Box<dyn Fn()>>,
    pub on_sent: Option<Box<dyn Fn(&EmailOptions, &str)>>,
    pub on_error: Option<Box<dyn Fn(Option<&EmailOptions>, &dyn std::error::Error)>>,
    pub on_close: Option<Box<dyn Fn(Option<&worker::Error>)>>,
}

impl Clone for WorkerMailerHooks {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for WorkerMailerHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WorkerMailerHooks")
    }
}

/// DSN options (global).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DsnOptions {
    pub ret: Option<DsnRet>,
    pub notify: Option<DsnNotify>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DsnRet {
    pub headers: Option<bool>,
    pub full: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DsnNotify {
    pub delay: Option<bool>,
    pub failure: Option<bool>,
    pub success: Option<bool>,
}

/// Options to create WorkerMailer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerMailerOptions {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub secure: bool,
    #[serde(default = "default_start_tls")]
    pub start_tls: bool,
    pub credentials: Option<Credentials>,
    #[serde(default)]
    pub auth_type: Vec<AuthType>,
    #[serde(default)]
    pub log_level: LogLevel,
    pub dsn: Option<DsnOptions>,
    #[serde(default = "default_socket_timeout_ms")]
    pub socket_timeout_ms: u64,
    #[serde(default = "default_response_timeout_ms")]
    pub response_timeout_ms: u64,
    #[serde(skip)]
    pub hooks: WorkerMailerHooks,
}

fn default_start_tls() -> bool {
    true
}
fn default_socket_timeout_ms() -> u64 {
    60_000
}
fn default_response_timeout_ms() -> u64 {
    30_000
}

impl Default for WorkerMailerOptions {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 587,
            secure: false,
            start_tls: true,
            credentials: None,
            auth_type: vec![],
            log_level: LogLevel::Info,
            dsn: None,
            socket_timeout_ms: 60_000,
            response_timeout_ms: 30_000,
            hooks: WorkerMailerHooks::default(),
        }
    }
}

/// SMTP client using Cloudflare Workers TCP Socket.
pub struct WorkerMailer {
    socket: Option<Socket>,
    host: String,
    port: u16,
    secure: bool,
    start_tls: bool,
    auth_type: Vec<AuthType>,
    credentials: Option<Credentials>,
    logger: Logger,
    dsn: Option<DsnOptions>,
    response_timeout_ms: u64,
    hooks: WorkerMailerHooks,
    supports_dsn: bool,
    allow_auth: bool,
    auth_type_supported: Vec<AuthType>,
    supports_start_tls: bool,
}

impl WorkerMailer {
    /// Connect to SMTP server and perform EHLO/STARTTLS/AUTH.
    pub async fn connect(options: WorkerMailerOptions) -> Result<Self, worker::Error> {
        let socket = ConnectionBuilder::new()
            .allow_half_open(false)
            .connect(options.host.clone(), options.port)?;

        let mut mailer = Self {
            socket: Some(socket),
            host: options.host.clone(),
            port: options.port,
            secure: options.secure,
            start_tls: options.start_tls,
            auth_type: options.auth_type,
            credentials: options.credentials,
            logger: Logger::new(
                options.log_level,
                format!("[WorkerMailer:{}:{}]", options.host, options.port),
            ),
            dsn: options.dsn,
            response_timeout_ms: options.response_timeout_ms,
            hooks: options.hooks,
            supports_dsn: false,
            allow_auth: false,
            auth_type_supported: vec![],
            supports_start_tls: false,
        };

        mailer.initialize_smtp_session().await?;
        if let Some(ref f) = mailer.hooks.on_connect {
            f();
        }
        Ok(mailer)
    }

    /// Send one email (connect, send, close).
    pub async fn send(
        options: WorkerMailerOptions,
        email_options: EmailOptions,
    ) -> Result<(), worker::Error> {
        let mut mailer = Self::connect(options).await?;
        mailer.send_one(email_options).await?;
        mailer.close(None).await
    }

    /// Send one email on this connection.
    pub async fn send_one(
        &mut self,
        email_options: EmailOptions,
    ) -> Result<String, worker::Error> {
        let mut email = Email::new(email_options.clone()).map_err(|e| {
            worker::Error::RustError(match e {
                crate::email::EmailBuildError::InvalidContent(ic) => ic.to_string(),
                crate::email::EmailBuildError::InvalidEmail(ie) => ie.to_string(),
            })
        })?;

        self.cmd_mail(&email).await?;
        self.cmd_rcpt(&email).await?;
        self.cmd_data().await?;
        let body = email.get_email_data();
        self.write(&body).await?;
        let response = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !response.starts_with('2') {
            return Err(worker::Error::RustError(format!("Failed to send body: {}", response)));
        }
        if let Some(ref f) = self.hooks.on_sent {
            f(&email_options, &response);
        }
        Ok(response)
    }

    async fn initialize_smtp_session(&mut self) -> Result<(), worker::Error> {
        self.greet().await?;
        self.ehlo().await?;

        if self.start_tls && !self.secure && self.supports_start_tls {
            let s = self.socket.take().unwrap();
            self.socket = Some(s.start_tls());
            self.ehlo().await?;
        }

        self.auth().await?;
        Ok(())
    }

    async fn read_response(&mut self) -> Result<String, String> {
        let mut buf = vec![0u8; 4096];
        let mut response = String::new();
        loop {
            let n = self.socket.as_mut().unwrap().read(&mut buf).await.map_err(|e| format!("read error: {}", e))?;
            if n == 0 {
                break;
            }
            let s = decode(&buf[..n]).map_err(|e| e.to_string())?;
            self.logger.debug(&format!("SMTP response:\n{}", s));
            response.push_str(&s);
            if !response.ends_with('\n') {
                continue;
            }
            let lines: Vec<&str> = response.lines().collect();
            let last = lines.iter().rev().nth(1).unwrap_or(&"");
            if last.len() >= 4 {
                let _code = &last[..3];
                let cont = last.chars().nth(3).unwrap_or(' ');
                if cont == '-' {
                    continue;
                }
            }
            break;
        }
        Ok(response)
    }

    async fn write_line(&mut self, line: &str) -> Result<(), worker::Error> {
        self.write(&format!("{}\r\n", line)).await
    }

    async fn write(&mut self, data: &str) -> Result<(), worker::Error> {
        self.logger.debug(&format!("Write:\n{}", data));
        let bytes = encode(data);
        self.socket.as_mut().unwrap().write_all(&bytes).await?;
        self.socket.as_mut().unwrap().flush().await?;
        Ok(())
    }

    async fn greet(&mut self) -> Result<(), worker::Error> {
        let response = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !response.starts_with("220") {
            return Err(worker::Error::RustError(format!(
                "Failed to connect: {}",
                response
            )));
        }
        Ok(())
    }

    async fn ehlo(&mut self) -> Result<(), worker::Error> {
        self.write_line("EHLO 127.0.0.1").await?;
        let response = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if response.starts_with("421") {
            return Err(worker::Error::RustError(format!("EHLO failed: {}", response)));
        }
        if !response.starts_with('2') {
            self.helo().await?;
            return Ok(());
        }
        self.parse_capabilities(&response);
        Ok(())
    }

    async fn helo(&mut self) -> Result<(), worker::Error> {
        self.write_line("HELO 127.0.0.1").await?;
        let response = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !response.starts_with('2') {
            return Err(worker::Error::RustError(format!("HELO failed: {}", response)));
        }
        Ok(())
    }

    fn parse_capabilities(&mut self, response: &str) {
        if response.to_uppercase().contains("AUTH") {
            self.allow_auth = true;
        }
        if response.to_uppercase().contains("AUTH") && response.to_uppercase().contains("PLAIN") {
            self.auth_type_supported.push(AuthType::Plain);
        }
        if response.to_uppercase().contains("AUTH") && response.to_uppercase().contains("LOGIN") {
            self.auth_type_supported.push(AuthType::Login);
        }
        if response.to_uppercase().contains("AUTH") && response.to_uppercase().contains("CRAM-MD5") {
            self.auth_type_supported.push(AuthType::CramMd5);
        }
        if response.to_uppercase().contains("STARTTLS") {
            self.supports_start_tls = true;
        }
        if response.to_uppercase().contains("DSN") {
            self.supports_dsn = true;
        }
    }

    async fn auth(&mut self) -> Result<(), worker::Error> {
        if !self.allow_auth {
            return Ok(());
        }
        let creds = match self.credentials.clone() {
            Some(c) => c,
            None => return Err(worker::Error::RustError("Auth required but no credentials".into())),
        };

        if self.auth_type_supported.contains(&AuthType::Plain) && self.auth_type.contains(&AuthType::Plain) {
            self.auth_plain(&creds).await?;
        } else if self.auth_type_supported.contains(&AuthType::Login) && self.auth_type.contains(&AuthType::Login) {
            self.auth_login(&creds).await?;
        } else if self.auth_type_supported.contains(&AuthType::CramMd5) && self.auth_type.contains(&AuthType::CramMd5) {
            self.auth_cram_md5(&creds).await?;
        } else {
            return Err(worker::Error::RustError("No supported auth method".into()));
        }
        Ok(())
    }

    async fn auth_plain(&mut self, creds: &Credentials) -> Result<(), worker::Error> {
        let blob = format!("\u{0}{}\u{0}{}", creds.username, creds.password);
        let b64 = B64.encode(blob.as_bytes());
        self.write_line(&format!("AUTH PLAIN {}", b64)).await?;
        let r = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !r.starts_with('2') {
            return Err(worker::Error::RustError(format!("AUTH PLAIN failed: {}", r)));
        }
        Ok(())
    }

    async fn auth_login(&mut self, creds: &Credentials) -> Result<(), worker::Error> {
        self.write_line("AUTH LOGIN").await?;
        let r = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !r.starts_with('3') {
            return Err(worker::Error::RustError(format!("AUTH LOGIN: {}", r)));
        }
        let u = B64.encode(creds.username.as_bytes());
        self.write_line(&u).await?;
        let r = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !r.starts_with('3') {
            return Err(worker::Error::RustError(format!("AUTH LOGIN user: {}", r)));
        }
        let p = B64.encode(creds.password.as_bytes());
        self.write_line(&p).await?;
        let r = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !r.starts_with('2') {
            return Err(worker::Error::RustError(format!("AUTH LOGIN: {}", r)));
        }
        Ok(())
    }

    async fn auth_cram_md5(&mut self, _creds: &Credentials) -> Result<(), worker::Error> {
        self.write_line("AUTH CRAM-MD5").await?;
        let r = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        let rest = r.strip_prefix("334 ").unwrap_or("").trim();
        let _challenge = B64
            .decode(rest)
            .map_err(|_| worker::Error::RustError("Invalid CRAM-MD5 challenge".into()))?;
        // HMAC-MD5 in WASM would need a crate (e.g. hmac + md5). Use PLAIN or LOGIN for now.
        return Err(worker::Error::RustError(
            "CRAM-MD5 not fully implemented in this example; use PLAIN or LOGIN".into(),
        ));
    }

    async fn cmd_mail(&mut self, email: &Email) -> Result<(), worker::Error> {
        let mut msg = format!("MAIL FROM: <{}>", email.from.email);
        if self.supports_dsn {
            msg.push_str(" ");
            // optional RET= and ENVID
        }
        self.write_line(&msg).await?;
        let r = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !r.starts_with('2') {
            return Err(worker::Error::RustError(format!("MAIL FROM failed: {}", r)));
        }
        Ok(())
    }

    async fn cmd_rcpt(&mut self, email: &Email) -> Result<(), worker::Error> {
        let mut all = email.to.clone();
        if let Some(ref cc) = email.cc {
            all.extend(cc.iter().cloned());
        }
        if let Some(ref bcc) = email.bcc {
            all.extend(bcc.iter().cloned());
        }
        for user in &all {
            let line = format!("RCPT TO: <{}>", user.email);
            self.write_line(&line).await?;
            let r = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
            if !r.starts_with('2') {
                return Err(worker::Error::RustError(format!(
                    "RCPT TO failed for {}: {}",
                    user.email, r
                )));
            }
        }
        Ok(())
    }

    async fn cmd_data(&mut self) -> Result<(), worker::Error> {
        self.write_line("DATA").await?;
        let r = self.read_response().await.map_err(|e| worker::Error::RustError(e))?;
        if !r.starts_with('3') {
            return Err(worker::Error::RustError(format!("DATA failed: {}", r)));
        }
        Ok(())
    }

    /// Close the connection.
    pub async fn close(&mut self, _error: Option<worker::Error>) -> Result<(), worker::Error> {
        let _ = self.write_line("QUIT").await;
        let _ = self.read_response().await;
        if let Some(ref mut s) = self.socket {
            s.close().await?;
        }
        if let Some(ref f) = self.hooks.on_close {
            f(None);
        }
        Ok(())
    }
}
