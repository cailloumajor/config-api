use tokio::sync::{mpsc, oneshot};
use tracing::{error, instrument};
use trillium::{Conn, Handler, KnownHeaderName, State, Status};
use trillium_api::ApiConnExt;
use trillium_router::{Router, RouterConnExt};

use crate::db::{GetConfigMessage, GetConfigRequest, GetConfigResponse};

#[derive(Clone)]
struct AppState {
    health_channel: mpsc::Sender<oneshot::Sender<bool>>,
    get_config_channel: mpsc::Sender<GetConfigMessage>,
}

async fn remove_server_header(mut conn: Conn) -> Conn {
    conn.headers_mut().remove(KnownHeaderName::Server);
    conn
}

pub(crate) fn handler(
    health_channel: mpsc::Sender<oneshot::Sender<bool>>,
    get_config_channel: mpsc::Sender<GetConfigMessage>,
) -> impl Handler {
    (
        State::new(AppState {
            health_channel,
            get_config_channel,
        }),
        remove_server_header,
        Router::new()
            .get("/health", health_handler)
            .get("/config/:collection/:id", get_config_handler),
    )
}

#[instrument(name = "health_api_handler", skip_all)]
async fn health_handler(conn: Conn) -> Conn {
    let (tx, rx) = oneshot::channel();
    let request_channel = &conn.state::<AppState>().unwrap().health_channel;
    if let Err(err) = request_channel.try_send(tx) {
        error!(kind = "request channel sending", %err);
        return conn.with_status(Status::InternalServerError).halt();
    }
    match rx.await {
        Ok(true) => conn.with_status(Status::NoContent).halt(),
        Ok(false) => conn.with_status(Status::InternalServerError).halt(),
        Err(err) => {
            error!(kind = "outcome channel receiving", %err);
            conn.with_status(Status::InternalServerError).halt()
        }
    }
}

#[instrument(name = "config_api_handler", skip_all)]
async fn get_config_handler(conn: Conn) -> Conn {
    let (tx, rx) = oneshot::channel();
    let request_channel = &conn.state::<AppState>().unwrap().get_config_channel;
    let request = GetConfigRequest {
        collection: conn.param("collection").unwrap().into(),
        id: conn.param("id").unwrap().into(),
    };
    if let Err(err) = request_channel.try_send((request, tx)) {
        error!(kind = "request channel sending", %err);
        return conn.with_status(Status::InternalServerError).halt();
    }
    match rx.await {
        Ok(GetConfigResponse::Document(doc)) => conn.with_json(&doc),
        Ok(GetConfigResponse::NotFound(message)) => {
            conn.with_status(Status::NotFound).with_body(message).halt()
        }
        Ok(GetConfigResponse::Error) => conn.with_status(Status::InternalServerError).halt(),
        Err(err) => {
            error!(during = "document finding", %err);
            conn.with_status(Status::InternalServerError).halt()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use trillium_testing::prelude::*;

    use super::*;

    mod health_handler {
        use super::*;

        fn testing_app(health_channel: mpsc::Sender<oneshot::Sender<bool>>) -> impl Handler {
            let (get_config_channel, _) = mpsc::channel(1);
            handler(health_channel, get_config_channel)
        }

        #[test]
        fn request_sending_error() {
            let (tx, _) = mpsc::channel(1);
            let app = testing_app(tx);
            let conn = get("/health").on(&app);
            assert_status!(&conn, Status::InternalServerError);
        }

        #[test]
        fn outcome_channel_receiving_error() {
            let (tx, mut rx) = mpsc::channel(1);
            thread::spawn(move || {
                // Consume and drop the response channel
                let _ = rx.blocking_recv().expect("channel has been closed");
            });
            let app = testing_app(tx);
            let conn = get("/health").on(&app);
            assert_status!(&conn, Status::InternalServerError);
        }

        #[test]
        fn unhealthy() {
            let (tx, mut rx) = mpsc::channel::<oneshot::Sender<bool>>(1);
            thread::spawn(move || {
                let response_tx = rx.blocking_recv().expect("channel has been closed");
                response_tx.send(false).expect("error sending response");
            });
            let app = testing_app(tx);
            let conn = get("/health").on(&app);
            assert_status!(&conn, Status::InternalServerError);
        }

        #[test]
        fn healthy() {
            let (tx, mut rx) = mpsc::channel::<oneshot::Sender<bool>>(1);
            thread::spawn(move || {
                let response_tx = rx.blocking_recv().expect("channel has been closed");
                response_tx.send(true).expect("error sending response");
            });
            let app = testing_app(tx);
            let conn = get("/health").on(&app);
            assert_status!(&conn, Status::NoContent);
        }
    }

    mod get_config_handler {
        use mongodb::bson::doc;

        use super::*;

        fn testing_app(get_config_channel: mpsc::Sender<GetConfigMessage>) -> impl Handler {
            let (health_channel, _) = mpsc::channel(1);
            handler(health_channel, get_config_channel)
        }

        #[test]
        fn request_sending_error() {
            let (tx, _) = mpsc::channel(1);
            let app = testing_app(tx);
            let conn = get("/config/somecoll/someid").on(&app);
            assert_status!(&conn, Status::InternalServerError);
        }

        #[test]
        fn outcome_channel_receiving_error() {
            let (tx, mut rx) = mpsc::channel(1);
            thread::spawn(move || {
                // Consume and drop the response channel
                let _ = rx.blocking_recv().expect("channel has been closed");
            });
            let app = testing_app(tx);
            let conn = get("/config/somecoll/someid").on(&app);
            assert_status!(&conn, Status::InternalServerError);
        }

        #[test]
        fn error_response() {
            let (tx, mut rx) = mpsc::channel::<GetConfigMessage>(1);
            thread::spawn(move || {
                let (_, response_tx) = rx.blocking_recv().expect("channel has been closed");
                response_tx
                    .send(GetConfigResponse::Error)
                    .expect("error sending response");
            });
            let app = testing_app(tx);
            let conn = get("/config/somecoll/someid").on(&app);
            assert_status!(&conn, Status::InternalServerError);
        }

        #[test]
        fn not_found_response() {
            let (tx, mut rx) = mpsc::channel::<GetConfigMessage>(1);
            thread::spawn(move || {
                let (request, response_tx) = rx.blocking_recv().expect("channel has been closed");
                response_tx
                    .send(GetConfigResponse::NotFound(format!("{request:?}")))
                    .expect("error sending response");
            });
            let app = testing_app(tx);
            let mut conn = get("/config/somecoll/someid").on(&app);
            assert_response!(
                &mut conn,
                Status::NotFound,
                r#"GetConfigRequest { collection: "somecoll", id: "someid" }"#
            );
        }

        #[test]
        fn document_response() {
            let (tx, mut rx) = mpsc::channel::<GetConfigMessage>(1);
            thread::spawn(move || {
                let (request, response_tx) = rx.blocking_recv().expect("channel has been closed");
                let document = doc! {
                    "collection": request.collection.as_str(),
                    "id": request.id.as_str(),
                };
                response_tx
                    .send(GetConfigResponse::Document(document))
                    .expect("error sending response");
            });
            let app = testing_app(tx);
            let mut conn = get("/config/somecoll/someid").on(&app);
            assert_response!(
                &mut conn,
                Status::Ok,
                r#"{"collection":"somecoll","id":"someid"}"#,
                "Content-Type" => "application/json",
            );
        }
    }
}
