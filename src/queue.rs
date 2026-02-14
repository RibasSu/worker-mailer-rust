//! Queue handler for processing emails via Cloudflare Queues (mirror of TS queue).

use crate::email::EmailOptions;
use crate::mailer::{WorkerMailer, WorkerMailerOptions};
use worker::{MessageBatch, MessageExt};

/// Message format for the email queue.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueEmailMessage {
    pub mailer_options: WorkerMailerOptions,
    pub email_options: EmailOptions,
}

/// Result of processing a queued email.
#[derive(Debug, Clone)]
pub struct QueueProcessResult {
    pub success: bool,
    pub error: Option<String>,
    pub email_options: EmailOptions,
}

/// Process a message batch and return results.
pub async fn process_batch(
    batch: MessageBatch<QueueEmailMessage>,
) -> Vec<QueueProcessResult> {
    let messages = batch.messages().unwrap_or_default();
    let mut results = Vec::new();
    for message in messages {
        let mailer_options = message.body().mailer_options.clone();
        let email_options = message.body().email_options.clone();
        match WorkerMailer::send(mailer_options, email_options.clone()).await {
            Ok(_) => {
                message.ack();
                results.push(QueueProcessResult {
                    success: true,
                    error: None,
                    email_options,
                });
            }
            Err(e) => {
                message.retry();
                results.push(QueueProcessResult {
                    success: false,
                    error: Some(e.to_string()),
                    email_options,
                });
            }
        }
    }
    results
}

/// Enqueue one email (send to the queue).
pub async fn enqueue_email(
    queue: &worker::Queue,
    message: &QueueEmailMessage,
) -> Result<(), worker::Error> {
    queue.send(message).await
}

/// Enqueue multiple emails.
pub async fn enqueue_emails(
    queue: &worker::Queue,
    messages: &[QueueEmailMessage],
) -> Result<(), worker::Error> {
    queue.send_batch(messages.to_vec()).await
}
