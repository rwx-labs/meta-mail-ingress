use axum::{Router, extract::DefaultBodyLimit, routing::get};
use serde::{Deserialize, Serialize};
use serde_with::{hex::Hex, serde_as};
use sqlx::prelude::FromRow;
use time::OffsetDateTime;
use tracing::error;
use uuid::Uuid;

use crate::{AppState, Error};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct UserUpload {
    pub id: Uuid,
    pub name: String,
    #[serde_as(as = "Hex")]
    pub sha256sum: Vec<u8>,
    pub size: i64,
    pub path: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(handlers::list_uploads).post(handlers::create_upload),
        )
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
}

async fn get_user_uploads(user_id: i32, db: &crate::Database) -> Result<Vec<UserUpload>, Error> {
    let addrs: Vec<UserUpload> =
        sqlx::query_as("SELECT * FROM user_file_uploads WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(db)
            .await
            .inspect_err(|err| error!(%user_id, "could not read users uploads: {err}"))
            .map_err(|_| Error::InternalError)?;

    Ok(addrs)
}

// async fn stream_to_temp_file<S, E>(stream: S) -> Result<(), (StatusCode, String)>
// where
//     S: Stream<Item = Result<Bytes, E>>,
//     E: Into<BoxError>,
// {
//     async {
//         // Convert the stream into an `AsyncRead`.
//         let body_with_io_error = stream.map_err(io::Error::other);
//         let body_reader = StreamReader::new(body_with_io_error);
//         futures::pin_mut!(body_reader);
//
//         // Create the file. `File` implements `AsyncWrite`.
//         let mut file = NamedTempFile::new().expect("could not create temp file");
//
//         // Copy the body into the file.
//         tokio::io::copy(&mut body_reader, &mut file).await?;
//
//         Ok::<_, io::Error>(())
//     }
//     .await
//     .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
// }

mod handlers {
    use axum::{
        extract::{Multipart, State},
        http::StatusCode,
        response::{IntoResponse, Json},
    };
    use tracing::{debug, error};

    use crate::{AppState, auth::AuthSession};

    pub(crate) async fn create_upload(
        auth_session: AuthSession,
        State(AppState { database: _, .. }): State<AppState>,
        mut multipart: Multipart,
    ) -> Result<(), (StatusCode, String)> {
        match auth_session.user {
            Some(_user) => {
                while let Some(field) = multipart
                    .next_field()
                    .await
                    .inspect_err(|err| error!("could not read next multipart field: {err}"))
                    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?
                {
                    let name = field.name().unwrap().to_string();
                    let file_name = field.file_name().unwrap().to_string();
                    let content_type = field.content_type().unwrap().to_string();

                    debug!(?name, ?file_name, ?content_type, "multipart");
                }

                Ok(())
            }
            None => Err((StatusCode::UNAUTHORIZED, String::new())),
        }
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
