use std::fmt;

use axum_login::AuthUser;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    /// The unique ID of the user.
    pub id: i32,
    /// The users e-mail address.
    pub email: String,
    /// The users access token.
    pub access_token: String,
}

impl fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
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
