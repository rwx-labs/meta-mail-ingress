use openidconnect::core::CoreRequestTokenError;

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
