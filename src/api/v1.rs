use axum::{Router};

use crate::AppState;

mod mail;

pub fn router() -> Router<AppState> {
    let mail_router = mail::router();

    Router::new().nest("/mails", mail_router)
}
