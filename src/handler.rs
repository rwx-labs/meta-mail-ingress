use std::{
    fs,
    io::{self, Write},
};

use aws_sdk_s3::{primitives::ByteStream, types::ObjectCannedAcl};
use mail_parser::Message;
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempPath;
use tracing::{debug, info, instrument};

use crate::{
    postprocess::{self, PostProcessor},
    Error,
};

#[derive(Debug)]
pub struct MailHandler {
    /// The number of e-mails that have been processed.
    pub num_mails_processed: u64,
    /// The number of attachments that have been processed.
    pub num_attachments_processed: u64,
    /// The number of attachment bytes that have been processed.
    pub num_attachments_bytes_processed: u64,
    /// List of registered post processors.
    pub processors: Vec<Box<dyn PostProcessor>>,
    /// AWS S3 client.
    pub s3_client: aws_sdk_s3::Client,
    /// Meta webhook bearer token.
    pub meta_webhook_token: String,
}

impl MailHandler {
    pub fn new(s3_client: aws_sdk_s3::Client, meta_webhook_token: String) -> Self {
        MailHandler {
            num_attachments_bytes_processed: 0,
            num_attachments_processed: 0,
            num_mails_processed: 0,
            processors: postprocess::init(),
            s3_client,
            meta_webhook_token,
        }
    }

    #[instrument(skip_all)]
    pub async fn handle(&mut self, mail: Message<'_>) -> Result<(), Error> {
        if mail.attachment_count() == 0 {
            info!(from = ?mail.from(), "skipping email as it doesn't contain any attachments");

            return Ok(());
        }

        let subject = mail.subject();

        for attachment in mail.attachments() {
            debug!("processing attachment");

            let attachment_size = attachment.len();
            let mime_type = tree_magic_mini::from_u8(attachment.contents());
            let mut file = tempfile::NamedTempFile::new().expect("could not create temp file"); // .map_err(Error::CreateTempFile)?;

            debug!(
                path = %file.path().display(),
                "writing {attachment_size} bytes attachment of type {mime_type} to disk"
            );

            file.write_all(attachment.contents())?;

            // Run post-processing pipeline on the temporary file.
            let mut path = Some(file.into_temp_path());
            let processors = self.processors.iter().filter(|x| x.applicable(mime_type));

            for processor in processors {
                if let Some(inner_path) = path {
                    path = match processor.apply(inner_path) {
                        Ok(temp_path) => Some(temp_path),
                        Err(_) => None,
                    }
                }
            }

            if let Some(inner_path) = path {
                let _ = self.upload_attachment(inner_path, mime_type, subject).await;
            }

            self.num_attachments_processed += 1;
            self.num_attachments_bytes_processed += attachment_size as u64;
        }

        self.num_mails_processed += 1;

        Ok(())
    }

    #[instrument(skip_all)]
    async fn send_chat_message(&mut self, msg: String) -> Result<(), Error> {
        let payload = json!({
            "method": "message",
            "params": {
                "network": "irc.rwx.im:6697",
                "channel": "#uplink",
                "message": msg
            }
        });

        let client = reqwest::Client::new();
        let res = client
            .post("https://meta-webhook.infra.rwx.im/trigger")
            .header(
                "Authorization",
                format!("Bearer {}", self.meta_webhook_token),
            )
            .json(&payload)
            .send()
            .await?
            .text()
            .await?;

        debug!(%res, "sent chat message");

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn upload_attachment(
        &mut self,
        path: TempPath,
        mime_type: &str,
        subject: Option<&str>,
    ) -> Result<String, Error> {
        let hash_bytes = {
            let mut hasher = Sha256::new();
            let mut file = fs::File::open(&path)?;
            let _bytes_written = io::copy(&mut file, &mut hasher)?;

            hasher.finalize()
        };

        debug!("uploading object with key {:x}", hash_bytes);

        let key = format!("~meta/mails/v1/{:x}", hash_bytes);
        let put_object = self
            .s3_client
            .put_object()
            .bucket("rwx-pub")
            .acl(ObjectCannedAcl::PublicRead)
            .key(&key)
            .content_type(mime_type)
            .content_disposition("inline");

        let body = ByteStream::from_path(&path).await;

        if let Ok(b) = body {
            let _result = put_object
                .body(b)
                .send()
                .await
                .map_err(|e| Error::S3PutObjectFailed(Box::new(e.into())))?;

            if let Some(subject) = subject {
                let _ = self
                    .send_chat_message(format!(
                        "\x0310> Mail received (\x0f{subject}\x0310) https://pub.rwx.im/{key}"
                    ))
                    .await;
            } else {
                let _ = self
                    .send_chat_message(format!("\x0310> Mail received https://pub.rwx.im/{key}"))
                    .await;
            }
        }

        Ok("".to_string())
    }
}
