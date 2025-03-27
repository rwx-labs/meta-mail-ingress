use std::{
    fs,
    io::{self, Write},
};

use aws_sdk_s3::{primitives::ByteStream, types::ObjectCannedAcl, Error as AwsS3Error};
use base64::prelude::{Engine, BASE64_URL_SAFE_NO_PAD};
use mail_parser::Message;
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::{NamedTempFile, TempPath};
use tracing::{debug, error, info, instrument};

use crate::{config::AwsS3Config, postprocess::PostProcessor, Error};

/// The result of an attachment upload.
pub struct AttachmentUpload {
    /// The key of the stored object.
    pub key: String,
    /// The subject of the e-mail, if any.
    pub subject: Option<String>,
    /// The sender of the e-mail.
    pub sender: Option<String>,
    /// Whether the attachment already existed in the remote bucket.
    pub cached: bool,
}

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
    /// AWS S3 configuration.
    pub s3_config: AwsS3Config,
    /// Meta webhook bearer token.
    pub meta_webhook_token: String,
}

impl MailHandler {
    #[must_use]
    pub fn new(
        s3_client: aws_sdk_s3::Client,
        s3_config: AwsS3Config,
        meta_webhook_token: String,
        postprocessors: Vec<Box<dyn PostProcessor>>,
    ) -> Self {
        MailHandler {
            num_attachments_bytes_processed: 0,
            num_attachments_processed: 0,
            num_mails_processed: 0,
            processors: postprocessors,
            s3_client,
            s3_config,
            meta_webhook_token,
        }
    }

    #[instrument(skip_all)]
    pub async fn handle(&mut self, mail: Message<'_>, from: Option<&str>) -> Result<(), Error> {
        if mail.attachment_count() == 0 {
            info!(from = ?mail.from(), "skipping email as it doesn't contain any attachments");

            return Ok(());
        }

        let subject = mail.subject();

        for attachment in mail.attachments() {
            debug!("processing attachment");

            let attachment_size = attachment.len();
            let mime_type = tree_magic_mini::from_u8(attachment.contents());
            let mut file = NamedTempFile::new().expect("could not create temp file");

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
                let sender = from;

                match self
                    .upload_attachment(inner_path, mime_type, subject, sender)
                    .await
                {
                    Ok(upload) => {
                        let key = upload.key;
                        let sender = upload.sender.unwrap_or("unknown".to_string());
                        let message = match subject {
                            Some(subject) => {
                                format!("\x0310> “\x0f{subject}\x0310” from\x0f {sender}\x0310: https://pub.rwx.im/{key}")
                            }
                            None => {
                                format!("\x0310> Mail received from\x0f {sender}\x0310 https://pub.rwx.im/{key}")
                            }
                        };

                        if let Err(err) = self.send_chat_message(message).await {
                            error!(%err, "could not send notification message");
                        }
                    }
                    Err(err) => {
                        error!(%err, %mime_type, ?subject, ?sender, "could not upload attachment");
                    }
                }
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

    /// Returns whether the object with the given `key` already exists in the S3 bucket.
    pub async fn object_exists(&mut self, key: &str) -> Result<bool, Error> {
        match self
            .s3_client
            .head_object()
            .bucket(&self.s3_config.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| e.into())
        {
            Ok(_) => Ok(true),
            Err(AwsS3Error::NotFound(_)) => Ok(false),
            Err(e) => Err(Error::AwsS3Error(Box::new(e))),
        }
    }

    #[instrument(skip_all)]
    pub async fn upload_attachment(
        &mut self,
        path: TempPath,
        mime_type: &str,
        subject: Option<&str>,
        sender: Option<&str>,
    ) -> Result<AttachmentUpload, Error> {
        let sha256_bytes = {
            let mut hasher = Sha256::new();
            let mut file = fs::File::open(&path)?;
            let _ = io::copy(&mut file, &mut hasher)?;

            hasher.finalize()
        };

        let encoded_hash = BASE64_URL_SAFE_NO_PAD.encode(sha256_bytes);
        let key = format!("~meta/mails/v2/{encoded_hash}");

        // Check if the file already exists.
        if let Ok(true) = self.object_exists(&key).await {
            debug!(%key, "skipping upload of object as it already exists in the bucket");

            return Ok(AttachmentUpload {
                key,
                sender: sender.map(String::from),
                subject: subject.map(String::from),
                cached: true,
            });
        }

        debug!(%key, "uploading object");

        let put_object = self
            .s3_client
            .put_object()
            .bucket(&self.s3_config.bucket_name)
            .acl(ObjectCannedAcl::PublicRead)
            .key(&key)
            .content_type(mime_type)
            .content_disposition(content_type_disposition(mime_type));

        match ByteStream::from_path(&path).await {
            Ok(body) => {
                let _ = put_object
                    .body(body)
                    .send()
                    .await
                    .map_err(|e| Error::S3PutObjectFailed(Box::new(e.into())))?;

                return Ok(AttachmentUpload {
                    key,
                    sender: sender.map(String::from),
                    subject: subject.map(String::from),
                    cached: true,
                });
            }
            Err(err) => {
                return Err(Error::ByteStream(Box::new(err)));
            }
        }
    }
}

/// Returns the content disposition based on the given `content_type`.
#[allow(clippy::match_same_arms)]
fn content_type_disposition(content_type: &str) -> &'static str {
    match content_type {
        "image/jpeg" | "image/png" | "image/heic" | "image/webp" | "image/gif" => "inline",
        "video/mp4" | "video/mpeg" | "video/ogg" | "video/webm" => "inline",
        _ => "attachment",
    }
}
