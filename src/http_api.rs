use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::{routing, Json, Router};
use reqwest::StatusCode;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, instrument};

use crate::db::{GetConfigMessage, GetConfigRequest, GetConfigResponse};

type HealthChannel = mpsc::Sender<oneshot::Sender<bool>>;
type GetConfigChannel = mpsc::Sender<GetConfigMessage>;

const INTERNAL_ERROR: (StatusCode, &str) =
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error");

impl IntoResponse for GetConfigResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            GetConfigResponse::Document(doc) => Json(doc).into_response(),
            GetConfigResponse::NotFound(message) => {
                (StatusCode::NOT_FOUND, message).into_response()
            }
            GetConfigResponse::Error => INTERNAL_ERROR.into_response(),
        }
    }
}

#[derive(Debug, Clone)]
struct AppState {
    health_channel: HealthChannel,
    get_config_channel: GetConfigChannel,
}

pub(crate) fn app(health_channel: HealthChannel, get_config_channel: GetConfigChannel) -> Router {
    Router::new()
        .route("/health", routing::get(health_handler))
        .route("/config/:collection/:id", routing::get(get_config_handler))
        .with_state(AppState {
            health_channel,
            get_config_channel,
        })
}

#[instrument(name = "health_api_handler", skip_all)]
async fn health_handler(State(state): State<AppState>) -> Result<StatusCode, impl IntoResponse> {
    let (tx, rx) = oneshot::channel();
    state.health_channel.try_send(tx).map_err(|err| {
        error!(kind = "request channel sending", %err);
        INTERNAL_ERROR
    })?;
    rx.await
        .map_err(|err| {
            error!(kind = "outcome channel receiving", %err);
            INTERNAL_ERROR
        })?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(INTERNAL_ERROR)
}

#[instrument(name = "config_api_handler", skip_all)]
async fn get_config_handler(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
) -> Result<GetConfigResponse, impl IntoResponse> {
    let request = GetConfigRequest { collection, id };
    let (tx, rx) = oneshot::channel();
    state
        .get_config_channel
        .try_send((request, tx))
        .map_err(|err| {
            error!(kind = "request channel sending", %err);
            INTERNAL_ERROR
        })?;
    rx.await.map_err(|err| {
        error!(kind = "outcome channel receiving", %err);
        INTERNAL_ERROR
    })
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;

    mod health_handler {
        use super::*;

        fn testing_fixture(
            health_channel: mpsc::Sender<oneshot::Sender<bool>>,
        ) -> (Router, Request<Body>) {
            let (get_config_channel, _) = mpsc::channel(1);
            let app = app(health_channel, get_config_channel);
            let req = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap();
            (app, req)
        }

        #[tokio::test]
        async fn request_sending_error() {
            let (tx, _) = mpsc::channel(1);
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn outcome_channel_receiving_error() {
            let (tx, mut rx) = mpsc::channel(1);
            tokio::spawn(async move {
                // Consume and drop the response channel
                let _ = rx.recv().await.expect("channel has been closed");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn unhealthy() {
            let (tx, mut rx) = mpsc::channel::<oneshot::Sender<bool>>(1);
            tokio::spawn(async move {
                let response_tx = rx.recv().await.expect("channel has been closed");
                response_tx.send(false).expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn healthy() {
            let (tx, mut rx) = mpsc::channel::<oneshot::Sender<bool>>(1);
            tokio::spawn(async move {
                let response_tx = rx.recv().await.expect("channel has been closed");
                response_tx.send(true).expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::NO_CONTENT);
        }
    }

    mod get_config_handler {
        use mongodb::bson::doc;

        use super::*;

        fn testing_fixture(get_config_channel: GetConfigChannel) -> (Router, Request<Body>) {
            let (health_channel, _) = mpsc::channel(1);
            let app = app(health_channel, get_config_channel);
            let req = Request::builder()
                .uri("/config/somecoll/someid")
                .body(Body::empty())
                .unwrap();
            (app, req)
        }

        #[tokio::test]
        async fn request_sending_error() {
            let (tx, _) = mpsc::channel(1);
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn outcome_channel_receiving_error() {
            let (tx, mut rx) = mpsc::channel(1);
            tokio::spawn(async move {
                // Consume and drop the response channel
                let _ = rx.recv().await.expect("channel has been closed");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn error_response() {
            let (tx, mut rx) = mpsc::channel::<GetConfigMessage>(1);
            tokio::spawn(async move {
                let (_, response_tx) = rx.recv().await.expect("channel has been closed");
                response_tx
                    .send(GetConfigResponse::Error)
                    .expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn not_found_response() {
            let (tx, mut rx) = mpsc::channel::<GetConfigMessage>(1);
            tokio::spawn(async move {
                let (request, response_tx) = rx.recv().await.expect("channel has been closed");
                response_tx
                    .send(GetConfigResponse::NotFound(format!("{request:?}")))
                    .expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
            let body = hyper::body::to_bytes(res).await.unwrap();
            assert_eq!(
                body,
                r#"GetConfigRequest { collection: "somecoll", id: "someid" }"#
            );
        }

        #[tokio::test]
        async fn document_response() {
            let (tx, mut rx) = mpsc::channel::<GetConfigMessage>(1);
            tokio::spawn(async move {
                let (request, response_tx) = rx.recv().await.expect("channel has been closed");
                let document = doc! {
                    "collection": request.collection.as_str(),
                    "id": request.id.as_str(),
                };
                response_tx
                    .send(GetConfigResponse::Document(document))
                    .expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(res.headers()["Content-Type"], "application/json");
            let body = hyper::body::to_bytes(res).await.unwrap();
            assert_eq!(body, r#"{"collection":"somecoll","id":"someid"}"#);
        }
    }
}
