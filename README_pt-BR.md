# Worker Mailer (Rust)

[English](./README.md) | [Português](./README_pt-BR.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Worker Mailer é um cliente SMTP para **Cloudflare Workers** escrito em **Rust**. É uma porta do [@ribassu/worker-mailer](https://github.com/RibasSu/worker-mailer) (TypeScript) e utiliza [Cloudflare TCP Sockets](https://developers.cloudflare.com/workers/runtime-apis/tcp-sockets/) através do crate [`worker`](https://docs.rs/worker).

## Funcionalidades

- 🚀 Feito para o runtime Cloudflare Workers (compila para `wasm32-unknown-unknown`)
- 📧 Envio de emails em texto puro e HTML com anexos
- 🖼️ Anexos de imagem inline com suporte a Content-ID (CID)
- 🔒 Autenticação SMTP: **PLAIN** e **LOGIN** (CRAM-MD5 não implementado nesta porta)
- ✅ Validação de endereços de email (compatível com RFC 5322)
- 🎯 Tipos de erro customizados para melhor tratamento
- 🪝 Hooks de ciclo de vida para monitorar operações
- 📅 Suporte a DSN (estruturas de opções)
- 📬 Integração com Cloudflare Queues para processamento assíncrono

## Índice

- [Instalação](#instalação)
- [Início Rápido](#início-rápido)
- [Referência da API](#referência-da-api)
- [Imagens Inline (CID)](#imagens-inline-cid)
- [Hooks de Ciclo de Vida](#hooks-de-ciclo-de-vida)
- [Tratamento de Erros](#tratamento-de-erros)
- [Integração com Cloudflare Queues](#integração-com-cloudflare-queues)
- [Limitações](#limitações)
- [Build para Workers](#build-para-workers)
- [Licença](#licença)

## Instalação

Adicione no `Cargo.toml` do seu Worker:

```toml
[dependencies]
worker = { version = "0.7", features = ["queue"] }
worker-mailer = { path = "../worker-mailer-rust" }
# ou pelo crates.io quando publicado:
# worker-mailer = "0.1"
```

## Início Rápido

1. Certifique-se de que o projeto do Worker usa o target `wasm32-unknown-unknown` e o crate `worker` com a feature `queue` se for usar Queues.

2. Conecte e envie um email:

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
        host: "smtp.exemplo.com".to_string(),
        port: 587,
        secure: false,
        start_tls: true,
        credentials: Some(Credentials {
            username: "seu-usuario-smtp".to_string(),
            password: "sua-senha-smtp".to_string(),
        }),
        auth_type: vec![AuthType::Plain],
        log_level: LogLevel::Info,
        ..Default::default()
    };

    let email_options = EmailOptions {
        from: Recipient::Email("de@exemplo.com".to_string()),
        to: vec![Recipient::Email("para@exemplo.com".to_string())],
        subject: "Olá do Worker Mailer".to_string(),
        text: Some("Esta é uma mensagem em texto puro.".to_string()),
        html: Some("<h1>Olá</h1><p>Esta é HTML.</p>".to_string()),
        ..Default::default()
    };

    WorkerMailer::send(options, email_options).await?;
    worker::Response::ok("OK")
}
```

3. Em produção, use secrets (ex.: `env.secret("SMTP_USER")`) em vez de credenciais fixas no código.

## Referência da API

### WorkerMailer::connect(options)

Cria uma nova conexão SMTP. Retorna um `WorkerMailer` que pode enviar vários emails na mesma conexão.

```rust
pub struct WorkerMailerOptions {
    pub host: String,
    pub port: u16,
    pub secure: bool,           // Usar TLS (padrão: false)
    pub start_tls: bool,        // Atualizar para TLS se suportado (padrão: true)
    pub credentials: Option<Credentials>,
    pub auth_type: Vec<AuthType>, // ex.: vec![AuthType::Plain]
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

Envia um email em uma conexão já aberta.

```rust
let mut mailer = WorkerMailer::connect(options).await?;
mailer.send_one(email_options).await?;
mailer.close(None).await?;
```

### WorkerMailer::send(options, email_options)

Envia um único email sem manter a conexão (conecta, envia, fecha).

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
    pub content: String,      // Conteúdo em Base64
    pub mime_type: Option<String>,
    pub cid: Option<String>,  // Content-ID para imagens inline
    pub inline: Option<bool>,
}
```

## Imagens Inline (CID)

Incorpore imagens em emails HTML usando Content-ID (CID):

```rust
use worker_mailer::{WorkerMailer, WorkerMailerOptions, EmailOptions, Recipient, Attachment};

let mailer = WorkerMailer::connect(options).await?;

mailer.send_one(EmailOptions {
    from: Recipient::Email("remetente@exemplo.com".to_string()),
    to: vec![Recipient::Email("destinatario@exemplo.com".to_string())],
    subject: "Email com imagem incorporada".to_string(),
    html: Some(r#"
        <h1>Olá!</h1>
        <p>Aqui está nosso logo:</p>
        <img src="cid:logo-empresa" alt="Logo da Empresa">
    "#.to_string()),
    attachments: Some(vec![Attachment {
        filename: "logo.png".to_string(),
        content: logo_base64,
        mime_type: Some("image/png".to_string()),
        cid: Some("logo-empresa".to_string()),
        inline: Some(true),
    }]),
    ..Default::default()
}).await?;
```

## Hooks de Ciclo de Vida

Monitore as operações definindo hooks em `WorkerMailerOptions.hooks`:

```rust
use worker_mailer::WorkerMailerHooks;

let options = WorkerMailerOptions {
    hooks: WorkerMailerHooks {
        on_connect: Some(Box::new(|| {
            worker::console_log!("Conectado ao servidor SMTP");
        })),
        on_sent: Some(Box::new(|email, response| {
            worker::console_log!(format!("Email enviado: {}", response));
        })),
        on_error: Some(Box::new(|email, err| {
            worker::console_error!(format!("Falha no envio: {}", err));
        })),
        on_close: Some(Box::new(|err| {
            if let Some(e) = err {
                worker::console_error!(format!("Conexão fechada: {}", e));
            }
        })),
        ..Default::default()
    },
    ..suas_opcoes
};
```

## Tratamento de Erros

O crate usa `worker::Error` e tipos de erro específicos para falhas SMTP:

```rust
use worker_mailer::{WorkerMailer, WorkerMailerOptions, EmailOptions, Recipient};

match WorkerMailer::send(options, email_options).await {
    Ok(()) => worker::Response::ok("Enviado"),
    Err(e) => {
        let msg = e.to_string();
        if msg.contains("AUTH") {
            // Falha de autenticação
        } else if msg.contains("Invalid email") {
            // InvalidEmailError (ao construir Email)
        }
        worker::Response::error(msg, 500)
    }
}
```

Ao construir um `Email` com `Email::new(options)`, você pode obter `EmailBuildError::InvalidContent` (falta text/html) ou `EmailBuildError::InvalidEmail` (endereços inválidos).

## Integração com Cloudflare Queues

Para envio em alto volume ou assíncrono, use Cloudflare Queues.

### Configuração

1. Adicione produtor e consumer da Queue no `wrangler.toml`:

```toml
[[queues.producers]]
queue = "email-queue"
binding = "EMAIL_QUEUE"

[[queues.consumers]]
queue = "email-queue"
max_batch_size = 10
max_retries = 3
```

2. Implemente o handler da queue no seu worker:

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

3. Enfileire emails no handler de fetch:

```rust
use worker_mailer::{enqueue_email, enqueue_emails, QueueEmailMessage};

// Um email
enqueue_email(&env.queue("EMAIL_QUEUE")?, &QueueEmailMessage {
    mailer_options: options,
    email_options: email_opts,
}).await?;

// Vários
enqueue_emails(&env.queue("EMAIL_QUEUE")?, &[msg1, msg2]).await?;
```

## Limitações

- **Porta 25:** Cloudflare Workers não permite conexões de saída na porta 25. Use 587 ou 465.
- **Autenticação:** Apenas PLAIN e LOGIN estão implementados. CRAM-MD5 não (exigiria HMAC-MD5 em WASM).
- **Conexões:** Cada instância do Worker tem limites de conexões TCP simultâneas. Feche as conexões quando terminar (ex.: `mailer.close(None).await`).

## Build para Workers

```bash
# Com worker-build (recomendado)
cargo install worker-build
worker-build

# Ou manualmente
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
```

## Licença

Este projeto está licenciado sob a Licença MIT.
