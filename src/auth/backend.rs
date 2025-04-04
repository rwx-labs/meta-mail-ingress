use async_trait::async_trait;
use axum_login::{AuthnBackend, UserId};
use openidconnect::{AccessTokenHash, AuthorizationCode, OAuth2TokenResponse};
use tracing::{debug, error, trace};

use crate::auth::user::User;
use crate::auth::{AuthError, Authenticator, Credentials};

#[async_trait]
impl AuthnBackend for Authenticator {
    type User = User;
    type Error = AuthError;
    type Credentials = Credentials;

    #[tracing::instrument(skip(self, creds))]
    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        // Ensure the CSRF state has not been tampered with.
        if creds.old_state.secret() != creds.new_state.secret() {
            return Ok(None);
        }

        trace!(code = %creds.code,
            "requesting access token using authorization token");

        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(creds.code))
            .map_err(AuthError::OidcConfiguration)?
            .request_async(&self.http_client)
            .await
            .inspect_err(|err| {
                error!("could not exchange authorization token for access token: {err}")
            })
            .map_err(|_| AuthError::InvalidToken)?;

        let id_token_verifier = self.client.id_token_verifier();
        let id_token = token_response
            .extra_fields()
            .id_token()
            .ok_or(AuthError::InvalidToken)
            .inspect_err(|_| error!("token did not include id token"))?;
        let id_token_claims = id_token
            .claims(&id_token_verifier, &creds.nonce)
            .inspect_err(|err| error!(?err, "could not verify and extract token claims"))
            .map_err(AuthError::ClaimsVerification)?;

        // Verify the access token hash to ensure that the access token hasn't been substituted for
        // another user's.
        if let Some(expected_access_token_hash) = id_token_claims.access_token_hash() {
            let signing_alg = id_token.signing_alg().expect("signing algo");
            let signing_key = id_token
                .signing_key(&id_token_verifier)
                .expect("signing key");

            let actual_access_token_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                signing_alg,
                signing_key,
            )
            .inspect_err(|_| error!("the token signature is not valid"))
            .map_err(AuthError::TokenSignature)?;

            if actual_access_token_hash != *expected_access_token_hash {
                return Err(AuthError::InvalidAccessToken);
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
        .inspect_err(|e| error!(?e, "could not create user session"))
        .map_err(|_| Self::Error::InternalError)?;

        debug!("finished authenticating");

        Ok(Some(user))
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        Ok(sqlx::query_as("select * from users where id = $1")
            .bind(user_id)
            .fetch_optional(&self.db)
            .await
            .inspect_err(|e| error!(?e, %user_id, "could not get user"))
            .map_err(|_| Self::Error::InternalError)?)
    }
}
