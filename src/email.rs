//! Email building and MIME encoding (mirror of TS email module).

use crate::errors::{InvalidContentError, InvalidEmailError};
use crate::utils::{encode_header, encode_quoted_printable, is_valid_email};
use std::collections::HashMap;

/// Single recipient/sender with optional display name.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub email: String,
    pub name: Option<String>,
}

impl User {
    pub fn new(email: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: None,
        }
    }
    pub fn with_name(email: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: Some(name.into()),
        }
    }
}

/// Attachment (filename, base64 content, optional CID for inline).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content: String, // base64
    pub mime_type: Option<String>,
    pub cid: Option<String>,
    pub inline: Option<bool>,
}

/// DSN override per message.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DsnOverride {
    pub envelope_id: Option<String>,
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

/// Options to build an email (mirror of TS EmailOptions).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

impl Default for EmailOptions {
    fn default() -> Self {
        Self {
            from: Recipient::Email(String::new()),
            to: vec![],
            reply: None,
            cc: None,
            bcc: None,
            subject: String::new(),
            text: None,
            html: None,
            headers: None,
            attachments: None,
            dsn_override: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Recipient {
    Email(String),
    User(User),
}

/// Error when building an email.
#[derive(Debug)]
pub enum EmailBuildError {
    InvalidContent(InvalidContentError),
    InvalidEmail(InvalidEmailError),
}

impl std::fmt::Display for EmailBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmailBuildError::InvalidContent(e) => write!(f, "{}", e),
            EmailBuildError::InvalidEmail(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for EmailBuildError {}

impl From<String> for Recipient {
    fn from(s: String) -> Self {
        Recipient::Email(s)
    }
}

impl From<User> for Recipient {
    fn from(u: User) -> Self {
        Recipient::User(u)
    }
}

fn recipients_to_users(recipients: &[Recipient]) -> Vec<User> {
    recipients
        .iter()
        .map(|r| match r {
            Recipient::Email(e) => User::new(e.clone()),
            Recipient::User(u) => u.clone(),
        })
        .collect()
}
fn one_recipient_to_user(r: &Recipient) -> User {
    match r {
        Recipient::Email(e) => User::new(e.clone()),
        Recipient::User(u) => u.clone(),
    }
}

/// Built email with resolved headers and body.
pub struct Email {
    pub from: User,
    pub to: Vec<User>,
    pub reply: Option<User>,
    pub cc: Option<Vec<User>>,
    pub bcc: Option<Vec<User>>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Option<Vec<Attachment>>,
    pub dsn_override: Option<DsnOverride>,
    pub headers: HashMap<String, String>,
}

impl Email {
    pub fn new(options: EmailOptions) -> Result<Self, EmailBuildError> {
        if options.text.is_none() && options.html.is_none() {
            return Err(EmailBuildError::InvalidContent(InvalidContentError(
                "At least one of text or html must be provided".to_string(),
            )));
        }

        let from = one_recipient_to_user(&options.from);
        let to = recipients_to_users(&options.to);
        let reply = options.reply.map(|r| one_recipient_to_user(&r));
    let cc = options.cc.as_deref().map(recipients_to_users);
    let bcc = options.bcc.as_deref().map(recipients_to_users);

        let mut invalid = Vec::new();
        if !is_valid_email(&from.email) {
            invalid.push(from.email.clone());
        }
        for u in &to {
            if !is_valid_email(&u.email) {
                invalid.push(u.email.clone());
            }
        }
        if let Some(ref r) = reply {
            if !is_valid_email(&r.email) {
                invalid.push(r.email.clone());
            }
        }
        for list in [cc.as_deref(), bcc.as_deref()].into_iter().flatten() {
            for u in list {
                if !is_valid_email(&u.email) {
                    invalid.push(u.email.clone());
                }
            }
        }
        if !invalid.is_empty() {
            return Err(EmailBuildError::InvalidEmail(InvalidEmailError::new(
                format!("Invalid email address(es): {}", invalid.join(", ")),
                invalid,
            )));
        }

        let headers = options.headers.unwrap_or_default();

        Ok(Self {
            from,
            to,
            reply,
            cc,
            bcc,
            subject: options.subject,
            text: options.text,
            html: options.html,
            attachments: options.attachments,
            dsn_override: options.dsn_override,
            headers,
        })
    }

    fn generate_safe_boundary(prefix: &str) -> String {
        let mut bytes = [0u8; 28];
        getrandom::getrandom(&mut bytes).unwrap_or_default();
        let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        let boundary = format!("{}{}", prefix, hex);
        boundary
            .chars()
            .map(|c| {
                if "<>@,;:\\/[]?=\" ".contains(c) {
                    '_'
                } else {
                    c
                }
            })
            .collect()
    }

    fn get_mime_type(filename: &str) -> &'static str {
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
        match ext.as_str() {
            "txt" => "text/plain",
            "html" => "text/html",
            "csv" => "text/csv",
            "pdf" => "application/pdf",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
    }

    fn resolve_headers(&mut self) {
        if !self.headers.contains_key("From") {
            let from = if let Some(ref n) = self.from.name {
                format!("\"{}\" <{}>", encode_header(n), self.from.email)
            } else {
                self.from.email.clone()
            };
            self.headers.insert("From".to_string(), from);
        }
        if !self.headers.contains_key("To") {
            let to: String = self
                .to
                .iter()
                .map(|u| {
                    if let Some(ref n) = u.name {
                        format!("\"{}\" <{}>", encode_header(n), u.email)
                    } else {
                        u.email.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            self.headers.insert("To".to_string(), to);
        }
        if !self.headers.contains_key("Subject") {
            self.headers
                .insert("Subject".to_string(), encode_header(&self.subject));
        }
        if let Some(ref r) = self.reply {
            if !self.headers.contains_key("Reply-To") {
                let reply = if let Some(ref n) = r.name {
                    format!("\"{}\" <{}>", encode_header(n), r.email)
                } else {
                    r.email.clone()
                };
                self.headers.insert("Reply-To".to_string(), reply);
            }
        }
        if let Some(ref cc) = self.cc {
            if !self.headers.contains_key("Cc") {
                let cc_str: String = cc
                    .iter()
                    .map(|u| {
                        if let Some(ref n) = u.name {
                            format!("\"{}\" <{}>", encode_header(n), u.email)
                        } else {
                            u.email.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                self.headers.insert("Cc".to_string(), cc_str);
            }
        }
        if let Some(ref bcc) = self.bcc {
            if !self.headers.contains_key("Bcc") {
                let bcc_str: String = bcc
                    .iter()
                    .map(|u| {
                        if let Some(ref n) = u.name {
                            format!("\"{}\" <{}>", encode_header(n), u.email)
                        } else {
                            u.email.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                self.headers.insert("Bcc".to_string(), bcc_str);
            }
        }
        if !self.headers.contains_key("Date") {
            let now = worker::Date::now();
            self.headers.insert("Date".to_string(), now.to_string());
        }
        if !self.headers.contains_key("Message-ID") {
            let id = uuid::Uuid::new_v4();
            let domain = self.from.email.split('@').nth(1).unwrap_or("local");
            self.headers
                .insert("Message-ID".to_string(), format!("<{}@{}>", id, domain));
        }
    }

    fn apply_dot_stuffing(data: &str) -> String {
        let mut result = data.replace("\r\n.", "\r\n..");
        if result.starts_with('.') {
            result.insert(0, '.');
        }
        result
    }

    /// Build raw MIME message (including final CRLF.CRLF).
    pub fn get_email_data(&mut self) -> String {
        self.resolve_headers();

        let mut headers_vec = vec!["MIME-Version: 1.0".to_string()];
        for (k, v) in &self.headers {
            headers_vec.push(format!("{}: {}", k, v));
        }

        let mixed_boundary = Self::generate_safe_boundary("mixed_");
        let related_boundary = Self::generate_safe_boundary("related_");
        let alternative_boundary = Self::generate_safe_boundary("alternative_");

        let attachments = self.attachments.as_deref().unwrap_or(&[]);
        let inline_attachments: Vec<_> = attachments.iter().filter(|a| a.cid.is_some()).collect();
        let regular_attachments: Vec<_> = attachments.iter().filter(|a| a.cid.is_none()).collect();

        headers_vec.push(format!(
            "Content-Type: multipart/mixed; boundary=\"{}\"",
            mixed_boundary
        ));
        let headers = headers_vec.join("\r\n");

        let mut email_data = format!("{}\r\n\r\n", headers);
        email_data.push_str(&format!("--{}\r\n", mixed_boundary));

        if !inline_attachments.is_empty() {
            email_data.push_str(&format!(
                "Content-Type: multipart/related; boundary=\"{}\"\r\n\r\n",
                related_boundary
            ));
            email_data.push_str(&format!("--{}\r\n", related_boundary));
        }

        email_data.push_str(&format!(
            "Content-Type: multipart/alternative; boundary=\"{}\"\r\n\r\n",
            alternative_boundary
        ));

        if let Some(ref text) = self.text {
            email_data.push_str(&format!("--{}\r\n", alternative_boundary));
            email_data.push_str("Content-Type: text/plain; charset=\"UTF-8\"\r\n");
            email_data.push_str("Content-Transfer-Encoding: quoted-printable\r\n\r\n");
            email_data.push_str(&encode_quoted_printable(text, 76));
            email_data.push_str("\r\n\r\n");
        }
        if let Some(ref html) = self.html {
            email_data.push_str(&format!("--{}\r\n", alternative_boundary));
            email_data.push_str("Content-Type: text/html; charset=\"UTF-8\"\r\n");
            email_data.push_str("Content-Transfer-Encoding: quoted-printable\r\n\r\n");
            email_data.push_str(&encode_quoted_printable(html, 76));
            email_data.push_str("\r\n\r\n");
        }
        email_data.push_str(&format!("--{}--\r\n", alternative_boundary));

        for att in &inline_attachments {
            let mime = att
                .mime_type
                .as_deref()
                .unwrap_or_else(|| Self::get_mime_type(&att.filename));
            email_data.push_str(&format!("--{}\r\n", related_boundary));
            email_data.push_str(&format!(
                "Content-Type: {}; name=\"{}\"\r\n",
                mime, att.filename
            ));
            email_data.push_str("Content-Transfer-Encoding: base64\r\n");
            email_data.push_str(&format!("Content-ID: <{}>\r\n", att.cid.as_deref().unwrap_or("")));
            email_data.push_str(&format!(
                "Content-Disposition: inline; filename=\"{}\"\r\n\r\n",
                att.filename
            ));
            for chunk in att.content.as_bytes().chunks(72) {
                email_data.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                email_data.push_str("\r\n");
            }
            email_data.push_str("\r\n");
        }
        if !inline_attachments.is_empty() {
            email_data.push_str(&format!("--{}--\r\n", related_boundary));
        }

        for att in &regular_attachments {
            let mime = att
                .mime_type
                .as_deref()
                .unwrap_or_else(|| Self::get_mime_type(&att.filename));
            email_data.push_str(&format!("--{}\r\n", mixed_boundary));
            email_data.push_str(&format!(
                "Content-Type: {}; name=\"{}\"\r\n",
                mime, att.filename
            ));
            email_data.push_str(&format!("Content-Description: {}\r\n", att.filename));
            email_data.push_str(&format!(
                "Content-Disposition: attachment; filename=\"{}\";\r\n",
                att.filename
            ));
            let now = worker::Date::now();
            email_data.push_str(&format!("    creation-date=\"{}\";\r\n", now.to_string()));
            email_data.push_str("Content-Transfer-Encoding: base64\r\n\r\n");
            for chunk in att.content.as_bytes().chunks(72) {
                email_data.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                email_data.push_str("\r\n");
            }
            email_data.push_str("\r\n");
        }

        email_data.push_str(&format!("--{}--\r\n", mixed_boundary));

        let safe = Self::apply_dot_stuffing(&email_data);
        format!("{}\r\n.\r\n", safe)
    }
}

