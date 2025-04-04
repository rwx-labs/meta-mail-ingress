use axum::{Router, routing::post};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(handlers::create))
}

mod handlers {
    pub(crate) async fn create() {
        unimplemented!()
    }
}
