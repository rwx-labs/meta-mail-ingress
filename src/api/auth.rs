use axum::{Router, routing::get};

pub const NEXT_URL_KEY: &str = "auth.next-url";
pub const CSRF_STATE_KEY: &str = "auth.csrf-state";
pub const NONCE_KEY: &str = "auth.nonce";

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/login", get(handlers::login))
        .route("/logout", get(handlers::logout))
        .route("/callback", get(handlers::callback))
        .route("/userinfo", get(handlers::userinfo))
}

mod handlers {
    use axum::{
        extract::Query,
        http::StatusCode,
        response::{IntoResponse, Redirect},
    };
    use openidconnect::CsrfToken;
    use serde::Deserialize;
    use tower_sessions::Session;

    use tracing::{debug, error, instrument, trace};

    use crate::auth::AuthSession;

    #[derive(Debug, Deserialize)]
    pub struct AuthResponse {
        code: String,
        state: CsrfToken,
    }

    // This allows us to extract the "next" field from the query string. We use this
    // to redirect after log in.
    #[derive(Debug, Deserialize)]
    pub struct NextUrl {
        next: Option<String>,
    }

    #[instrument(skip_all)]
    pub(super) async fn login(
        auth_session: AuthSession,
        session: Session,
        Query(NextUrl { next }): Query<NextUrl>,
    ) -> impl IntoResponse {
        trace!("creating authorize url");
        let (auth_url, csrf_state, nonce) = auth_session.backend.authorize_url();

        trace!("setting auth session state");

        session
            .insert(super::CSRF_STATE_KEY, csrf_state.secret())
            .await
            .expect("unable to serialize");

        session
            .insert(super::NONCE_KEY, nonce.secret())
            .await
            .expect("unable to serialize");

        match next {
            Some(ref next) if next.starts_with('/') => {
                session
                    .insert(super::NEXT_URL_KEY, next)
                    .await
                    .expect("unable to serialize");
            }
            _ => {}
        }

        Redirect::to(auth_url.as_str())
    }

    #[instrument(skip_all)]
    pub(super) async fn logout(mut auth_session: AuthSession) -> impl IntoResponse {
        match auth_session.logout().await {
            Ok(_) => Redirect::to("/").into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }

    #[instrument(skip_all)]
    pub async fn userinfo(auth_session: AuthSession) -> impl IntoResponse {
        match auth_session.user {
            Some(user) => format!("{user:?}").into_response(),
            None => (StatusCode::INTERNAL_SERVER_ERROR, "no user").into_response(),
        }
    }

    #[instrument(skip_all)]
    pub async fn callback(
        mut auth_session: AuthSession,
        session: Session,
        Query(AuthResponse {
            state: new_state,
            code,
        }): Query<AuthResponse>,
    ) -> impl IntoResponse {
        let Ok(Some(old_state)) = session.get::<CsrfToken>(super::CSRF_STATE_KEY).await else {
            return (StatusCode::BAD_REQUEST, "missing csrf state").into_response();
        };

        let Ok(Some(nonce)) = session.get(super::NONCE_KEY).await else {
            return (StatusCode::BAD_REQUEST, "missing nonce").into_response();
        };

        debug!(old_state = %old_state.secret(), new_state = %new_state.secret(), "states");

        let creds = crate::auth::Credentials {
            code,
            nonce,
            old_state,
            new_state,
        };

        let user = match auth_session.authenticate(creds).await {
            Ok(Some(user)) => user,
            Ok(None) => return (StatusCode::UNAUTHORIZED, "invalid csrf state").into_response(),
            Err(err) => {
                error!(?err, "could not authenticate user");
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
            }
        };

        if auth_session.login(&user).await.is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "login failed").into_response();
        }

        if let Ok(Some(url)) = session.remove::<String>(super::NEXT_URL_KEY).await {
            Redirect::to(&url).into_response()
        } else {
            Redirect::to("/").into_response()
        }
    }
}
