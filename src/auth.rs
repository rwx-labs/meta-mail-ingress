//! OIDC authentication

use openidconnect::core::{
    CoreAuthDisplay, CoreAuthPrompt, CoreClient, CoreErrorResponseType, CoreGenderClaim,
    CoreIdTokenClaims, CoreJsonWebKey, CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm,
    CoreProviderMetadata, CoreRequestTokenError, CoreResponseType, CoreRevocableToken,
    CoreTokenType,
};
use openidconnect::{
    AuthenticationFlow, Client, ClientId, ClientSecret, CsrfToken, EmptyAdditionalClaims,
    EmptyExtraTokenFields, EndpointMaybeSet, EndpointNotSet, EndpointSet, IdTokenFields, IssuerUrl,
    Nonce, RedirectUrl, RevocationErrorResponseType, Scope, StandardErrorResponse,
    StandardTokenIntrospectionResponse, StandardTokenResponse, reqwest, url::Url,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

mod backend;
mod user;

use crate::Database;
use crate::Error;

pub(crate) type AuthClient = Client<
    EmptyAdditionalClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    StandardTokenResponse<
        IdTokenFields<
            EmptyAdditionalClaims,
            EmptyExtraTokenFields,
            CoreGenderClaim,
            CoreJweContentEncryptionAlgorithm,
            CoreJwsSigningAlgorithm,
        >,
        CoreTokenType,
    >,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, CoreTokenType>,
    CoreRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub type AuthSession = axum_login::AuthSession<Authenticator>;

/// HTTP client error.
pub type HttpClientError = reqwest::Error;

/// Errors that can occur during authentication.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Error encountered while requesting access token.
    #[error("could not exchange authorization token for access token")]
    RequestTokenError(#[source] CoreRequestTokenError<HttpClientError>),
    #[error("Could not configure OpenID Connect request")]
    OidcConfiguration(#[from] openidconnect::ConfigurationError),
    #[error("Invalid authentication token")]
    InvalidToken,
    /// The token did not include a valid nonce.
    #[error("invalid token nonce")]
    InvalidTokenNonce,
    /// The claim is invalid.
    #[error("Could not extract and verify token claims: {0}")]
    ClaimsVerification(#[from] openidconnect::ClaimsVerificationError),
    /// The token signature is not valid.
    #[error("Invalid token signature")]
    TokenSignature(#[from] openidconnect::SigningError),
    /// The access token is not valid.
    #[error("Invalid access token")]
    InvalidAccessToken,
    /// An internal error occurred that we don't want to disclose to the user.
    #[error("Internal error")]
    InternalError,
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
    #[error("Authentication error")]
    Auth(#[from] AuthError),
}

#[derive(Clone, Debug)]
pub struct Authenticator {
    db: Database,
    client: AuthClient,
    http_client: reqwest::Client,
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

        let http_client = reqwest::ClientBuilder::new()
            // Following redirects opens the client up to SSRF vulnerabilities.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Client should build");

        let issuer_url = IssuerUrl::from_url(url);
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client)
            .await
            .map_err(Error::OidcDiscovery)?;

        // Create an OpenID Connect client based on the parameters from the discovery and the
        // provided client id and secret.
        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
        )
        .set_redirect_uri(RedirectUrl::from_url(redirect_url));

        debug!("finished openid connect discovery");

        Ok(Authenticator {
            db,
            client,
            http_client,
        })
    }

    pub fn authorize_url(&self) -> (Url, CsrfToken, Nonce) {
        // Generate the full authorization URL
        let (auth_url, csrf_state, nonce) = self
            .client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            // Set the desired scopes.
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();

        (auth_url, csrf_state, nonce)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Claims(CoreIdTokenClaims);

impl AsRef<CoreIdTokenClaims> for Claims {
    fn as_ref(&self) -> &CoreIdTokenClaims {
        &self.0
    }
}
