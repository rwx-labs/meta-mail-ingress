use axum::Router;

use crate::AppState;

mod mail;
mod upload;

pub fn router() -> Router<AppState> {
    let mail_router = mail::router();
    let upload_router = upload::router();

    Router::new().nest("/mails", mail_router).nest("/uploads", upload_router)
}
