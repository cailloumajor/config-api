use trillium::{Conn, Status};

use crate::problem::{ProblemDetailsConnExt, PROBLEM_INVALID_CONFIG};
use crate::AppState;

pub async fn handler(conn: Conn) -> Conn {
    if conn
        .state::<AppState>()
        .unwrap()
        .static_config
        .read()
        .await
        .is_none()
    {
        return conn.with_problem_details(&PROBLEM_INVALID_CONFIG);
    }

    conn.with_status(Status::NoContent).halt()
}

#[cfg(test)]
mod tests {
    use async_std::sync::{Arc, RwLock};
    use trillium::{Handler, State};
    use trillium_caching_headers::EntityTag;
    use trillium_testing::prelude::*;

    use crate::{AppState, StaticConfig};

    fn handler(toml_value: Option<toml::Value>) -> impl Handler {
        let static_config = Arc::new(RwLock::new(toml_value.map(|t| StaticConfig {
            toml_value: t,
            etag: EntityTag::weak(""),
        })));
        (State::new(AppState { static_config }), super::handler)
    }

    #[async_std::test]
    async fn handler_none_static_config() {
        let handler = handler(None);
        let mut conn = get("/").on(&handler);
        assert_status!(conn, 500);
        assert_body_contains!(conn, r#""type":"/problem/config-invalid""#);
        assert_headers!(conn, "content-type" => "application/problem+json");
    }

    #[async_std::test]
    async fn handler_healthy() {
        let handler = handler(Some(toml::Value::Boolean(false)));
        assert_status!(get("/").on(&handler), 204);
    }
}
