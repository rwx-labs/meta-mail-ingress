use std::collections::HashMap;

use axum::{routing::post, Router};
use serde::Deserialize;
use tracing::info;

use crate::{http::AuthToken, AppState};

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MailMetadata {
    /// The intended recipient, if known.
    pub to: Option<String>,
    /// The sender, if known.
    pub from: Option<String>,
    /// E-mail headers, if known.
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Mail {
    /// The raw contents of the e-mail, encoded with base64.
    pub raw: String,
    /// The size of the (decoded) raw contents.
    pub raw_size: usize,
    /// Information about the e-mail that was known prior to parsing.
    pub metadata: MailMetadata,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
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

        info!(?payload, "ingesting");

        let mail_parser = MessageParser::new()
            .with_minimal_headers()
            .with_date_headers()
            .with_address_headers()
            .with_message_ids();

        for mail in &payload.mails {
            let Ok(decoded) = BASE64_STANDARD.decode(&mail.raw) else {
                continue;
            };

            match mail_parser.parse(&decoded[..]) {
                Some(parsed) => {
                    let from = mail.metadata.from.as_deref();
                    debug!(?from, ?parsed, "parsed mail");
                    let _ = mail_handler.lock().await.handle(parsed, from).await;
                }
                None => {
                    error!("could not parse email");
                }
            }
        }

        ().into_response()
    }
}
