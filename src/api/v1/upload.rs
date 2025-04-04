use axum::{Router, routing::get};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use tracing::error;

use crate::{AppState, Error};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Upload {
    pub filename: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/",
        get(handlers::list_uploads).post(handlers::create_upload),
    )
}

async fn get_user_uploads(user_id: i32, db: &crate::Database) -> Result<Vec<Upload>, Error> {
    let addrs: Vec<Upload> = sqlx::query_as("SELECT * FROM file_uploads WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(db)
        .await
        .inspect_err(|err| error!(%user_id, "could not read users uploads: {err}"))
        .map_err(|_| Error::InternalError)?;

    Ok(addrs)
}

mod handlers {
    use axum::{Json, extract::State, response::IntoResponse};
    use reqwest::StatusCode;

    use crate::{AppState, auth::AuthSession};

    pub(crate) async fn create_upload() {
        unimplemented!()
    }

    pub(crate) async fn list_uploads(
        auth_session: AuthSession,
        State(AppState { database, .. }): State<AppState>,
    ) -> impl IntoResponse {
        match auth_session.user {
            Some(user) => {
                if let Ok(uploads) = super::get_user_uploads(user.id, &database).await {
                    Json(uploads).into_response()
                } else {
                    (StatusCode::INTERNAL_SERVER_ERROR).into_response()
                }
            }
            None => (StatusCode::UNAUTHORIZED).into_response(),
        }
    }
}
