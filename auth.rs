//! OIDC authentication

use async_trait::async_trait;
use axum_login::{AuthUser, AuthnBackend, UserId};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    reqwest::AsyncHttpClientError,
    url::Url,
    AccessTokenHash, AuthorizationCode, ClaimsVerificationError, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, OAuth2TokenResponse, RedirectUrl, Scope,
};
use openidconnect::{
    core::{CoreIdTokenClaims, CoreRequestTokenError},
    reqwest::async_http_client,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::debug;

use crate::Database;
use crate::Error;

pub type AuthSession = axum_login::AuthSession<Authenticator>;

#[derive(Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub access_token: String,
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("email", &self.email)
            .field("access_token", &"[redacted]")
            .finish()
    }
}

impl AuthUser for User {
    type Id = i32;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.access_token.as_bytes()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    pub code: String,
    pub nonce: Nonce,
    pub old_state: CsrfToken,
    pub new_state: CsrfToken,
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error(transparent)]
    Sqlx(sqlx::Error),
    #[error("could not exchange authorization token for access token")]
    TokenExchangeFailed(#[source] CoreRequestTokenError<AsyncHttpClientError>),
    #[error("the received exchange token does not include expected fields")]
    InvalidToken,
    #[error("the received exchange tokens nonce does not match")]
    InvalidTokenNonce(#[source] ClaimsVerificationError),
    #[error("invalid access token")]
    InvalidAccessToken,
}

#[derive(Clone, Debug)]
pub struct Authenticator {
    db: Database,
    client: CoreClient,
}

impl Authenticator {
    /// Create an Authenticator based on the properties of an OpenID Connect Discovery document.
    pub(crate) async fn discover(
        db: Database,
        url: Url,
        client_id: String,
        client_secret: String,
        redirect_url: Url,
    ) -> Result<Self, Error> {
        debug!("running openid connect discovery");

        let issuer_url = IssuerUrl::from_url(url);
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
            .await
            .map_err(|_| Error::DiscoverOidcFailed)?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
        )
        .set_redirect_uri(RedirectUrl::from_url(redirect_url));

        debug!("finished openid connect discovery");

        Ok(Authenticator { db, client })
    }

    pub fn authorize_url(&self) -> (Url, CsrfToken, Nonce) {
        // Generate the full authorization URL
        let (auth_url, csrf_token, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            // Set the desired scopes.
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();

        (auth_url, csrf_token, nonce)
    }
}

#[async_trait]
impl AuthnBackend for Authenticator {
    type User = User;
    type Error = BackendError;
    type Credentials = Credentials;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        // Ensure the CSRF state has not been tampered with.
        if creds.old_state.secret() != creds.new_state.secret() {
            return Ok(None);
        };

        debug!(code = %creds.code, "requesting access token using authorization token");

        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(creds.code))
            .request_async(async_http_client)
            .await
            .map_err(BackendError::TokenExchangeFailed)?;

        debug!(?token_response);

        let id_token_verifier = self.client.id_token_verifier();
        let id_token = token_response
            .extra_fields()
            .id_token()
            .ok_or(BackendError::InvalidToken)?;
        let id_token_claims = id_token
            .claims(&id_token_verifier, &creds.nonce)
            .map_err(BackendError::InvalidTokenNonce)?;

        debug!(?id_token_claims);

        // Verify the access token hash to ensure that the access token hasn't been substituted for
        // another user's.
        if let Some(expected_access_token_hash) = id_token_claims.access_token_hash() {
            let actual_access_token_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                &id_token
                    .signing_alg()
                    .map_err(|_| BackendError::InvalidAccessToken)?,
            )
            .map_err(|_| BackendError::InvalidAccessToken)?;

            if actual_access_token_hash != *expected_access_token_hash {
                return Err(BackendError::InvalidAccessToken);
            }
        }
        let email = id_token_claims.email().expect("missing email").as_str();
        let access_token = token_response.access_token().secret();

        // Persist user in our database so we can use `get_user`.
        let user = sqlx::query_as(
            r"
            insert into users (email, access_token)
            values ($1, $2)
            on conflict(email) do update
            set access_token = excluded.access_token
            returning *
            ",
        )
        .bind(email)
        .bind(access_token)
        .fetch_one(&self.db)
        .await
        .map_err(Self::Error::Sqlx)?;

        debug!("finished authenticating");

        Ok(Some(user))
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        Ok(sqlx::query_as("select * from users where id = $1")
            .bind(user_id)
            .fetch_optional(&self.db)
            .await
            .map_err(Self::Error::Sqlx)?)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Claims(CoreIdTokenClaims);

impl AsRef<CoreIdTokenClaims> for Claims {
    fn as_ref(&self) -> &CoreIdTokenClaims {
        &self.0
    }
}
