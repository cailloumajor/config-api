use lazy_static::lazy_static;
use mongodb::bson::{doc, Document};
use mongodb::Database;
use tracing::{error, instrument};
use trillium::{Conn, Handler, KnownHeaderName, State, Status};
use trillium_api::ApiConnExt;
use trillium_router::{Router, RouterConnExt};

lazy_static! {
    static ref PING_COMMAND: Document = doc! { "ping": 1 };
}

#[derive(Clone)]
struct AppState {
    mongodb_database: Database,
}

async fn remove_server_header(mut conn: Conn) -> Conn {
    conn.headers_mut().remove(KnownHeaderName::Server);
    conn
}

pub(crate) fn handler(mongodb_database: Database) -> impl Handler {
    (
        State::new(AppState { mongodb_database }),
        remove_server_header,
        Router::new()
            .get("/health", health_handler)
            .get("/config/:collection/:id", config_handler),
    )
}

#[instrument(name = "health_api_handler", skip_all)]
async fn health_handler(conn: Conn) -> Conn {
    let database = &conn.state::<AppState>().unwrap().mongodb_database;
    match database.run_command(PING_COMMAND.clone(), None).await {
        Ok(_) => conn.with_status(Status::NoContent).halt(),
        Err(err) => {
            error!(during = "pinging database", %err);
            conn.with_status(Status::InternalServerError).halt()
        }
    }
}

#[instrument(name = "config_api_handler", skip_all)]
async fn config_handler(conn: Conn) -> Conn {
    let database = &conn.state::<AppState>().unwrap().mongodb_database;
    let collection_name = conn.param("collection").unwrap();
    let document_id = conn.param("id").unwrap();
    let collection = database.collection::<Document>(collection_name);
    let filter = doc! { "_id": document_id };
    match collection.find_one(filter, None).await {
        Ok(Some(doc)) => conn.with_json(&doc),
        Ok(None) => {
            let collection_name = collection_name.to_owned();
            let document_id = document_id.to_owned();
            conn.with_status(Status::NotFound)
                .with_body(format!(
                    "Document with id `{document_id}` not found in `{collection_name}` collection"
                ))
                .halt()
        }
        Err(err) => {
            error!(during = "document finding", %err);
            conn.with_status(Status::InternalServerError).halt()
        }
    }
}
