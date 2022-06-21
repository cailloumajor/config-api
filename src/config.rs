use anyhow::{ensure, Context};
use async_std::fs;
use serde_json::Value as JSONValue;
use toml::Value as TOMLValue;
use trillium::{Conn, Status};
use trillium_api::ApiConnExt;
use trillium_caching_headers::{CacheControlDirective, CachingHeadersExt, EntityTag};
use trillium_router::RouterConnExt;

use crate::problem::{ProblemDetails, ProblemDetailsConnExt, PROBLEM_INVALID_CONFIG};
use crate::{AppState, StaticConfig};

fn camel_case(source: &str) -> String {
    let mut dest = String::with_capacity(source.len());
    let mut capitalize = false;
    for ch in source.chars() {
        if ch == ' ' || ch == '_' {
            capitalize = !dest.is_empty();
        } else if capitalize {
            dest.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            dest.push(ch)
        }
    }
    dest
}

fn toml_to_json(toml_value: &TOMLValue) -> JSONValue {
    match toml_value {
        TOMLValue::String(s) => JSONValue::String(s.to_owned()),
        TOMLValue::Integer(i) => JSONValue::Number((*i).into()),
        TOMLValue::Float(f) => match serde_json::Number::from_f64(*f) {
            Some(n) => JSONValue::Number(n),
            None => JSONValue::Null,
        },
        TOMLValue::Boolean(b) => JSONValue::Bool(*b),
        TOMLValue::Datetime(dt) => JSONValue::String(dt.to_string()),
        TOMLValue::Array(arr) => JSONValue::Array(arr.iter().map(toml_to_json).collect()),
        TOMLValue::Table(table) => JSONValue::Object(
            table
                .into_iter()
                .map(|(k, v)| (camel_case(k), toml_to_json(v)))
                .collect(),
        ),
    }
}

pub async fn load_config(path: &str) -> anyhow::Result<StaticConfig> {
    let content = fs::read_to_string(path)
        .await
        .context("error reading configuration file")?;
    let metadata = fs::metadata(path)
        .await
        .context("error reading configuration file metadata")?;
    let etag = EntityTag::from_file_meta(&metadata);
    ensure!(!content.is_empty(), "configuration file is empty");
    let toml_value = content.parse()?;
    Ok(StaticConfig { toml_value, etag })
}

pub async fn handler(mut conn: Conn) -> Conn {
    let maybe_static_config = conn
        .state::<AppState>()
        .unwrap()
        .static_config
        .read()
        .await
        .to_owned();
    let StaticConfig { toml_value, etag } = match maybe_static_config {
        Some(static_config) => static_config,
        None => return conn.with_problem_details(&PROBLEM_INVALID_CONFIG),
    };
    let mut subset = &toml_value;
    conn.set_etag(&etag);
    conn.set_cache_control(CacheControlDirective::NoCache);
    if conn.if_none_match().map(|ref inm| etag.weak_eq(inm)) == Some(true) {
        return conn.with_status(Status::NotModified);
    }
    let path = conn.wildcard().unwrap().to_owned();
    for subpath in path.split('/') {
        match subset.get(subpath) {
            Some(value) => subset = value,
            None => {
                return conn.with_problem_details(
                    &ProblemDetails::new(
                        "path-not-found",
                        "path not found in configuration",
                        Status::NotFound,
                    )
                    .with_detail(&format!(
                        r"path `{}` was not found in static configuration",
                        path
                    )),
                )
            }
        };
    }
    conn.with_json(&toml_to_json(subset))
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::Path;

    use async_std::sync::{Arc, RwLock};
    use tempfile::NamedTempFile;
    use test_case::test_case;
    use trillium::{Handler, State};
    use trillium_caching_headers::EntityTag;
    use trillium_router::Router;
    use trillium_testing::prelude::*;

    use crate::{AppState, StaticConfig};

    macro_rules! tests_fixtures_dir {
        () => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/")
        };
    }

    const SNAPSHOTS_DIR: &str = concat!(tests_fixtures_dir!(), "snapshots");
    const TEST_TOML_PATH: &str = concat!(tests_fixtures_dir!(), "test.toml");
    const TEST_TOML: &str = include_str!("../tests/test.toml");

    #[test_case("oneword" => "oneword")]
    #[test_case("camelCase" => "camelCase")]
    #[test_case("space separated" => "spaceSeparated")]
    #[test_case("underscore_separated" => "underscoreSeparated")]
    #[test_case("multiple   spaces" => "multipleSpaces")]
    #[test_case("   spaces_underscores_mixed" => "spacesUnderscoresMixed")]
    fn camel_case(src: &str) -> String {
        super::camel_case(src)
    }

    #[test]
    fn toml_to_json() {
        let toml_value = TEST_TOML.parse().unwrap();
        let json_value = super::toml_to_json(&toml_value);
        let pretty_json = format!("{:#}", json_value);
        insta::with_settings!({snapshot_path => Path::new(SNAPSHOTS_DIR)}, {
            insta::assert_snapshot!(pretty_json);
        });
    }

    fn handler(toml_config: Option<&str>) -> impl Handler {
        let router = Router::new().get("/test/*", super::handler);
        let static_config = Arc::new(RwLock::new(toml_config.map(|t| StaticConfig {
            toml_value: toml::from_str(t).unwrap(),
            etag: EntityTag::weak("test-etag"),
        })));
        (State::new(AppState { static_config }), router)
    }

    #[async_std::test]
    async fn handler_none_static_config() {
        let handler = handler(None);
        let mut conn = get("/test/title").on(&handler);
        assert_status!(conn, 500);
        assert_body_contains!(conn, r#""type":"/problem/config-invalid""#);
        assert_headers!(conn, "content-type" => "application/problem+json");
    }

    #[async_std::test]
    async fn handler_etag_match() {
        let handler = handler(Some(TEST_TOML));
        let conn = get("/test/title")
            .with_request_header("If-None-Match", "W/\"test-etag\"")
            .on(&handler);
        assert_status!(conn, 304);
        assert_headers!(
            conn,
            "Cache-Control" => "no-cache",
            "Etag" => "W/\"test-etag\""
        )
    }

    #[async_std::test]
    async fn handler_nonexistent_path() {
        let handler = handler(Some(TEST_TOML));
        let mut conn = get("/test/nonexistent").on(&handler);
        assert_status!(conn, 404);
        assert_body_contains!(conn, r#""type":"/problem/path-not-found""#);
        assert_headers!(conn, "content-type" => "application/problem+json");
    }

    #[async_std::test]
    async fn handler_root_path() {
        let handler = handler(Some(TEST_TOML));
        let mut conn = get("/test/").on(&handler);
        assert_status!(conn, 404);
        assert_body_contains!(conn, r#""type":"/problem/path-not-found""#);
        assert_headers!(conn, "content-type" => "application/problem+json");
    }

    #[async_std::test]
    async fn handler_string_value() {
        let handler = handler(Some(TEST_TOML));
        assert_response!(
            get("/test/title").on(&handler),
            200,
            r#""TOML example 😊""#,
            "Content-Type" => "application/json",
            "Cache-Control" => "no-cache",
            "Etag" => "W/\"test-etag\""
        );
    }

    #[async_std::test]
    async fn handler_object_value() {
        let handler = handler(Some(TEST_TOML));
        assert_response!(
            get("/test/servers/alpha").on(&handler),
            200,
            r#"{"enabled":false,"hostname":"server1","ip":"10.0.0.1"}"#,
            "Content-Type" => "application/json",
            "Cache-Control" => "no-cache",
            "Etag" => "W/\"test-etag\""
        );
    }

    #[async_std::test]
    async fn load_config_no_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_owned();
        temp_file.close().unwrap();

        assert!(super::load_config(path.to_str().unwrap()).await.is_err());
    }

    #[async_std::test]
    async fn load_config_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();

        assert!(super::load_config(temp_file.path().to_str().unwrap())
            .await
            .is_err());
    }

    #[async_std::test]
    async fn load_config_parse_error() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "a = t").unwrap();

        assert!(super::load_config(temp_file.path().to_str().unwrap())
            .await
            .is_err());
    }

    #[async_std::test]
    async fn load_config_success() {
        assert!(super::load_config(TEST_TOML_PATH).await.is_ok());
    }
}
