use tide::{Request, Response, StatusCode};

use crate::AppState;

pub async fn handler(req: Request<AppState>) -> tide::Result {
    if req.state().static_config.read().await.is_none() {
        return Ok(Response::builder(500)
            .body("static configuration is invalid")
            .build());
    }

    Ok(StatusCode::NoContent.into())
}

#[cfg(test)]
mod tests {
    use async_std::sync::{Arc, RwLock};
    use tide::http::{Request, Response, Url};

    use crate::AppState;

    async fn request_handler(toml_value: Option<toml::Value>) -> Response {
        let static_config = Arc::new(RwLock::new(toml_value));
        let mut app = tide::with_state(AppState { static_config });
        app.at("/test").get(super::handler);
        let url = Url::parse("http://example.com/test").unwrap();
        app.respond(Request::get(url)).await.unwrap()
    }

    #[async_std::test]
    async fn handler_none_static_config() {
        let resp = request_handler(None).await;
        assert_eq!(resp.status(), 500);
    }

    #[async_std::test]
    async fn handler_healthy() {
        let resp = request_handler(Some(toml::Value::Boolean(false))).await;
        assert_eq!(resp.status(), 204);
    }
}
