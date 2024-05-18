use std::collections::HashMap;

use axum::{routing::post, Router};
use serde::Deserialize;

use crate::{http::AuthToken, AppState};

#[derive(Debug, Clone, Deserialize)]
pub struct MailMetadata {
    /// The intended recipient, if known.
    pub to: Option<String>,
    /// The sender, if known.
    pub from: Option<String>,
    /// E-mail headers, if known.
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mail {
    /// The raw contents of the e-mail, encoded with base64.
    pub raw: String,
    /// The size of the (decoded) raw contents.
    pub raw_size: usize,
    /// Information about the e-mail that was known prior to parsing.
    pub metadata: MailMetadata,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MailIngestionRequest {
    pub mails: Vec<Mail>,
    pub started_at: String, // FIXME: this should be deserialized to a time
}

pub fn router() -> Router<AppState> {
    Router::new().route("/ingestion", post(handlers::ingest))
}

mod handlers {
    use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
    use base64::prelude::{Engine, BASE64_STANDARD};
    use mail_parser::MessageParser;
    use tracing::{debug, error};

    use super::*;

    use crate::AppState;

    #[tracing::instrument(skip_all)]
    pub(super) async fn ingest(
        AuthToken(token): AuthToken,
        State(AppState {
            api_token,
            mail_handler,
        }): State<AppState>,
        Json(payload): Json<MailIngestionRequest>,
    ) -> impl IntoResponse {
        if token != api_token {
            return (StatusCode::UNAUTHORIZED, "invalid api token").into_response();
        }

        let mail_parser = MessageParser::new()
            .with_mime_headers()
            .with_date_headers()
            .with_address_headers()
            .with_message_ids();

        for mail in &payload.mails {
            let decoded = match BASE64_STANDARD.decode(&mail.raw) {
                Ok(data) => data,
                Err(_) => continue,
            };

            match mail_parser.parse(&decoded[..]) {
                Some(parsed) => {
                    debug!("parsed mail");
                    let _ = mail_handler.lock().await.handle(parsed).await;
                }
                None => {
                    error!("could not parse email");
                }
            }
        }

        ().into_response()
    }
}
