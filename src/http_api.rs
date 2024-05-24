use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::{routing, Json, Router};
use mongodb::bson::Bson;
use reqwest::StatusCode;
use tracing::{error, instrument};

use crate::db::{
    GetCollectionChannel, GetCollectionResponse, GetDocumentChannel, GetDocumentRequest,
    GetDocumentResponse, HealthChannel, PatchConfigChannel, PatchConfigRequest,
};

type HandlerError = (StatusCode, &'static str);

const INTERNAL_ERROR: HandlerError = (StatusCode::INTERNAL_SERVER_ERROR, "internal server error");

impl IntoResponse for GetCollectionResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            GetCollectionResponse::Documents(docs) => Json(docs).into_response(),
            GetCollectionResponse::NotFound(message) => {
                (StatusCode::NOT_FOUND, message).into_response()
            }
        }
    }
}

impl IntoResponse for GetDocumentResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            GetDocumentResponse::Document(doc) => Json(doc).into_response(),
            GetDocumentResponse::NotFound(message) => {
                (StatusCode::NOT_FOUND, message).into_response()
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) health_channel: HealthChannel,
    pub(crate) get_collection_channel: GetCollectionChannel,
    pub(crate) get_document_channel: GetDocumentChannel,
    pub(crate) patch_config_channel: PatchConfigChannel,
}

pub(crate) fn app(app_state: AppState) -> Router {
    Router::new()
        .route("/health", routing::get(health_handler))
        .route("/config/:collection", routing::get(get_collection_handler))
        .route(
            "/config/:collection/:id",
            routing::get(get_document_handler).patch(patch_config_handler),
        )
        .with_state(app_state)
}

#[instrument(name = "health_api_handler", skip_all)]
async fn health_handler(State(state): State<AppState>) -> Result<StatusCode, HandlerError> {
    state
        .health_channel
        .roundtrip(())
        .await
        .map_err(|err| {
            error!(kind = "health channel roundtrip", %err);
            INTERNAL_ERROR
        })?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(INTERNAL_ERROR)
}

#[instrument(name = "get_collection_api_handler", skip_all)]
async fn get_collection_handler(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<GetCollectionResponse, HandlerError> {
    state
        .get_collection_channel
        .roundtrip(collection)
        .await
        .map_err(|err| {
            error!(kind = "collection retrieve channel roundtrip", %err);
            INTERNAL_ERROR
        })
}

#[instrument(name = "get_document_api_handler", skip_all)]
async fn get_document_handler(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
) -> Result<GetDocumentResponse, HandlerError> {
    let request = GetDocumentRequest { collection, id };
    state
        .get_document_channel
        .roundtrip(request)
        .await
        .map_err(|err| {
            error!(kind = "document retrieve channel roundtrip", %err);
            INTERNAL_ERROR
        })
}

#[instrument(name = "patch_config_api_handler", skip_all)]
async fn patch_config_handler(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
    Json(changes): Json<HashMap<String, Bson>>,
) -> Result<StatusCode, HandlerError> {
    let request = PatchConfigRequest {
        collection,
        id,
        changes,
    };
    state
        .patch_config_channel
        .roundtrip(request)
        .await
        .map_err(|err| {
            error!(kind = "configuration patch channel roundtrip", %err);
            INTERNAL_ERROR
        })
}

#[cfg(test)]
mod tests {
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use mongodb::bson::doc;
    use tower::ServiceExt;

    use crate::channel::roundtrip_channel;

    use super::*;

    mod health_handler {
        use super::*;

        fn testing_fixture(health_channel: HealthChannel) -> (Router, Request<Body>) {
            let (get_collection_channel, _) = roundtrip_channel(1);
            let (get_document_channel, _) = roundtrip_channel(1);
            let (patch_config_channel, _) = roundtrip_channel(1);
            let app = app(AppState {
                health_channel,
                get_collection_channel,
                get_document_channel,
                patch_config_channel,
            });
            let req = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap();
            (app, req)
        }

        #[tokio::test]
        async fn roundtrip_error() {
            let (tx, _) = roundtrip_channel(1);
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn unhealthy() {
            let (tx, mut rx) = roundtrip_channel(1);
            tokio::spawn(async move {
                let (_, response_tx) = rx.recv().await.expect("channel has been closed");
                response_tx.send(false).expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn healthy() {
            let (tx, mut rx) = roundtrip_channel(1);
            tokio::spawn(async move {
                let (_, response_tx) = rx.recv().await.expect("channel has been closed");
                response_tx.send(true).expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::NO_CONTENT);
        }
    }

    mod get_collection_handler {
        use super::*;

        fn testing_fixture(
            get_collection_channel: GetCollectionChannel,
        ) -> (Router, Request<Body>) {
            let (health_channel, _) = roundtrip_channel(1);
            let (get_document_channel, _) = roundtrip_channel(1);
            let (patch_config_channel, _) = roundtrip_channel(1);
            let app = app(AppState {
                health_channel,
                get_collection_channel,
                get_document_channel,
                patch_config_channel,
            });
            let req = Request::builder()
                .uri("/config/somecollection")
                .body(Body::empty())
                .unwrap();
            (app, req)
        }

        #[tokio::test]
        async fn roundtrip_error() {
            let (tx, _) = roundtrip_channel(1);
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn not_found() {
            let (tx, mut rx) = roundtrip_channel(1);
            tokio::spawn(async move {
                let (request, response_tx) = rx.recv().await.expect("channel has been closed");
                response_tx
                    .send(GetCollectionResponse::NotFound(request))
                    .expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
            let body = to_bytes(res.into_body(), 1024).await.unwrap();
            assert_eq!(body, "somecollection");
        }

        #[tokio::test]
        async fn success() {
            let (tx, mut rx) = roundtrip_channel(1);
            tokio::spawn(async move {
                let (request, response_tx) = rx.recv().await.expect("channel has been closed");
                response_tx
                    .send(GetCollectionResponse::Documents(vec![
                        doc! { "a": 1, "b": "c" },
                        doc! { "a": 2, "b": request },
                    ]))
                    .expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(res.headers()["Content-Type"], "application/json");
            let body = to_bytes(res.into_body(), 1024).await.unwrap();
            assert_eq!(body, r#"[{"a":1,"b":"c"},{"a":2,"b":"somecollection"}]"#);
        }
    }

    mod get_document_handler {
        use super::*;

        fn testing_fixture(get_document_channel: GetDocumentChannel) -> (Router, Request<Body>) {
            let (health_channel, _) = roundtrip_channel(1);
            let (get_collection_channel, _) = roundtrip_channel(1);
            let (patch_config_channel, _) = roundtrip_channel(1);
            let app = app(AppState {
                health_channel,
                get_collection_channel,
                get_document_channel,
                patch_config_channel,
            });
            let req = Request::builder()
                .uri("/config/somecoll/someid")
                .body(Body::empty())
                .unwrap();
            (app, req)
        }

        #[tokio::test]
        async fn roundtrip_error() {
            let (tx, _) = roundtrip_channel(1);
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn not_found_response() {
            let (tx, mut rx) = roundtrip_channel(1);
            tokio::spawn(async move {
                let (request, response_tx) = rx.recv().await.expect("channel has been closed");
                response_tx
                    .send(GetDocumentResponse::NotFound(format!("{request:?}")))
                    .expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
            let body = to_bytes(res.into_body(), 1024).await.unwrap();
            assert_eq!(
                body,
                r#"GetDocumentRequest { collection: "somecoll", id: "someid" }"#
            );
        }

        #[tokio::test]
        async fn document_response() {
            let (tx, mut rx) = roundtrip_channel::<GetDocumentRequest, GetDocumentResponse>(1);
            tokio::spawn(async move {
                let (request, response_tx) = rx.recv().await.expect("channel has been closed");
                let document = doc! {
                    "collection": request.collection.as_str(),
                    "id": request.id.as_str(),
                };
                response_tx
                    .send(GetDocumentResponse::Document(document))
                    .expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(res.headers()["Content-Type"], "application/json");
            let body = to_bytes(res.into_body(), 1024).await.unwrap();
            assert_eq!(body, r#"{"collection":"somecoll","id":"someid"}"#);
        }
    }

    mod patch_config_handler {
        use super::*;

        fn testing_fixture(patch_config_channel: PatchConfigChannel) -> (Router, Request<Body>) {
            let (health_channel, _) = roundtrip_channel(1);
            let (get_collection_channel, _) = roundtrip_channel(1);
            let (get_document_channel, _) = roundtrip_channel(1);
            let app = app(AppState {
                health_channel,
                get_collection_channel,
                get_document_channel,
                patch_config_channel,
            });
            let req = Request::builder()
                .method("PATCH")
                .uri("/config/somecoll/someid")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"somekey":42,"otherkey":["a","b"]}"#))
                .unwrap();
            (app, req)
        }

        #[tokio::test]
        async fn roundtrip_error() {
            let (tx, _) = roundtrip_channel(1);
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn status_code_response() {
            let (tx, mut rx) = roundtrip_channel(1);
            tokio::spawn(async move {
                let (_, response_tx) = rx.recv().await.expect("channel has been closed");
                response_tx
                    .send(StatusCode::IM_A_TEAPOT)
                    .expect("error sending response");
            });
            let (app, req) = testing_fixture(tx);
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
        }
    }
}
