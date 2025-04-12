use axum::Router;
use axum_login::login_required;

use crate::AppState;
use crate::auth::Authenticator;

mod mail;
mod upload;

pub fn router() -> Router<AppState> {
    let mail_router = mail::router();
    let upload_router = upload::router();

    Router::new()
        .nest("/mails", mail_router)
        .nest("/uploads", upload_router)
        // The routes following this layer do not require login
        .route_layer(login_required!(Authenticator, login_url = "/auth/login"))
}
