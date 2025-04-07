use axum::Router;
use axum::response::Html;
use axum::routing::get;
use axum_login::login_required;

use crate::auth::Authenticator;
use crate::AppState;

mod mail;
mod upload;

async fn show_form() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head>
                <title>Upload something!</title>
            </head>
            <body>
                <form action="/v1/uploads" method="post" enctype="multipart/form-data">
                    <div>
                        <label>
                            Upload file:
                            <input type="file" name="file" multiple>
                        </label>
                    </div>

                    <div>
                        <input type="submit" value="Upload files">
                    </div>
                </form>
            </body>
        </html>
        "#,
    )
}

pub fn router() -> Router<AppState> {
    let mail_router = mail::router();
    let upload_router = upload::router();

    Router::new()
        .nest("/mails", mail_router)
        .nest("/uploads", upload_router)
        .route("/dashboard", get(show_form))
        // The routes following this layer do not require login
        .route_layer(login_required!(Authenticator, login_url = "/auth/login"))
}
