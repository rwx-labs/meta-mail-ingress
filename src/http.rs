use std::net::SocketAddr;

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use axum_login::AuthManagerLayerBuilder;
use listenfd::ListenFd;
use miette::IntoDiagnostic;
use time::Duration;
use tokio::{net::TcpListener, signal};
use tower_http::{
    compression::{CompressionLayer, CompressionLevel},
    trace::TraceLayer,
};
use tower_sessions::ExpiredDeletion;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing::{debug, instrument};

use crate::api;

pub struct AuthToken(pub String);

impl<S> FromRequestParts<S> for AuthToken
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        if let Some(auth) = parts.headers.get(AUTHORIZATION) {
            let value = auth.as_bytes();

            if value.starts_with(b"Token ") {
                let token = String::from_utf8_lossy(&value[6..]);

                Ok(AuthToken(token.into_owned()))
            } else {
                Err((StatusCode::BAD_REQUEST, "invalid authorization scheme"))
            }
        } else {
            Err((StatusCode::BAD_REQUEST, "authorization token is missing"))
        }
    }
}

#[instrument]
async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 page not found")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    println!("signal received, starting graceful shutdown");
}

/// Returns the health status of the server.
pub async fn healthcheck() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}

/// Start HTTP server.
#[instrument(skip_all)]
pub async fn start_server(state: crate::AppState) -> miette::Result<()> {
    debug!("starting http server");

    let api_v1_router = api::v1::router();
    let auth_router = api::auth::router();

    let session_store = state.session_store.clone();

    let _deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(tokio::time::Duration::from_secs(360)),
    );

    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_secure(false)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::seconds(360)));

    let auth_layer =
        AuthManagerLayerBuilder::new(state.authenticator.clone(), session_layer).build();

    let app = Router::new()
        .nest("/v1", api_v1_router)
        .nest("/auth", auth_router)
        .route("/livez", get(healthcheck))
        .route("/readyz", get(healthcheck))
        .with_state(state)
        .layer(auth_layer)
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new().quality(CompressionLevel::Fastest));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let mut listenfd = ListenFd::from_env();
    let listener = match listenfd.take_tcp_listener(0).unwrap() {
        // if we are given a tcp listener on listen fd 0, we use that one
        Some(listener) => {
            listener.set_nonblocking(true).unwrap();
            TcpListener::from_std(listener).unwrap()
        }
        // otherwise fall back to local listening
        None => {
            debug!("binding to {}", addr);

            TcpListener::bind(addr).await.into_diagnostic()?
        }
    };

    debug!("listening on {}", addr);
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .into_diagnostic()?;

    Ok(())
}
