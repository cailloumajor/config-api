use anyhow::{ensure, Context};
use async_std::fs;
use serde_json::Value as JSONValue;
use toml::Value as TOMLValue;
use trillium::{Conn, Status};
use trillium_api::ApiConnExt;
use trillium_caching_headers::{CacheControlDirective, CachingHeadersExt, EntityTag};
use trillium_router::RouterConnExt;

use static_config_api::rfc7807::{ProblemDetails, ProblemDetailsConnExt, PROBLEM_INVALID_CONFIG};

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
    let data = toml_to_json(&content.parse()?);
    Ok(StaticConfig { data, etag })
}

pub async fn handler(mut conn: Conn) -> Conn {
    let maybe_static_config = conn
        .state::<AppState>()
        .unwrap()
        .static_config
        .read()
        .await
        .to_owned();
    let StaticConfig { data, etag } = match maybe_static_config {
        Some(static_config) => static_config,
        None => return conn.with_problem_details(&PROBLEM_INVALID_CONFIG),
    };
    conn.set_etag(&etag);
    conn.set_cache_control(CacheControlDirective::NoCache);
    if conn.if_none_match().map(|ref inm| etag.weak_eq(inm)) == Some(true) {
        return conn.with_status(Status::NotModified);
    }
    let path = "/".to_owned() + conn.wildcard().unwrap();
    match data.pointer(&path) {
        Some(subset) => conn.with_json(subset),
        None => conn.with_problem_details(
            &ProblemDetails::new(
                "path-not-found",
                "path not found in configuration",
                Status::NotFound,
            )
            .with_detail(&format!(
                r"path `{}` was not found in static configuration",
                path
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use async_std::sync::{Arc, RwLock};
    use tempfile::NamedTempFile;
    use trillium::{Handler, State};
    use trillium_caching_headers::EntityTag;
    use trillium_router::Router;
    use trillium_testing::prelude::*;

    use crate::{AppState, StaticConfig};

    use super::*;

    macro_rules! tests_fixtures_dir {
        () => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/")
        };
    }

    const TEST_TOML_PATH: &str = concat!(tests_fixtures_dir!(), "test.toml");
    const TEST_JSON: &str = include_str!("../tests/test.json");

    mod camel_case {
        use super::*;

        #[test]
        fn one_word() {
            let result = camel_case("oneword");
            assert_eq!(result, "oneword");
        }

        #[test]
        fn already_camel_case() {
            let result = camel_case("camelCase");
            assert_eq!(result, "camelCase");
        }

        #[test]
        fn space_separated() {
            let result = camel_case("space separated");
            assert_eq!(result, "spaceSeparated");
        }

        #[test]
        fn underscore_separated() {
            let result = camel_case("underscore_separated");
            assert_eq!(result, "underscoreSeparated");
        }

        #[test]
        fn multiple_spaces() {
            let result = camel_case("multiple   spaces");
            assert_eq!(result, "multipleSpaces");
        }

        #[test]
        fn mixed_spaces_underscores() {
            let result = camel_case("   spaces_underscores_mixed");
            assert_eq!(result, "spacesUnderscoresMixed");
        }
    }

    mod toml_to_json {
        use std::collections::HashMap;

        use toml::value::Datetime;

        use super::*;

        #[test]
        fn string() {
            let json_value = toml_to_json(&"some_string".to_string().into());
            assert_eq!(json_value.as_str().unwrap(), "some string");
        }

        #[test]
        fn integer() {
            let json_value = toml_to_json(&42.into());
            assert_eq!(json_value.as_i64().unwrap(), 42);
        }

        #[test]
        fn normal_float() {
            let json_value = toml_to_json(&(37.5).into());
            assert_eq!(json_value.as_f64().unwrap(), 37.5);
        }

        #[test]
        fn anormal_float() {
            let json_value = toml_to_json(&f64::NAN.into());
            assert!(json_value.as_null().is_some());
        }

        #[test]
        fn boolean() {
            let json_value = toml_to_json(&true.into());
            assert_eq!(json_value.as_bool().unwrap(), true);
        }

        #[test]
        fn datetime() {
            let dt = "1984-12-09T04:30:00Z".parse::<Datetime>().unwrap();
            let json_value = toml_to_json(&dt.into());
            assert_eq!(json_value.as_str().unwrap(), "1984-12-09T04:30:00Z");
        }

        #[test]
        fn array() {
            let json_value = toml_to_json(&vec![true].into());
            assert_eq!(json_value.as_array().unwrap()[0].as_bool().unwrap(), true);
        }

        #[test]
        fn table() {
            let map = HashMap::from([("some_key", true)]);
            let json_value = toml_to_json(&map.into());
            assert_eq!(
                json_value.as_object().unwrap()["someKey"]
                    .as_bool()
                    .unwrap(),
                true
            );
        }
    }

    fn handler(json_config: Option<&str>) -> impl Handler {
        let router = Router::new().get("/test/*", super::handler);
        let static_config = Arc::new(RwLock::new(json_config.map(|t| StaticConfig {
            data: serde_json::from_str(t).unwrap(),
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
        let handler = handler(Some(TEST_JSON));
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
        let handler = handler(Some(TEST_JSON));
        let mut conn = get("/test/nonexistent").on(&handler);
        assert_status!(conn, 404);
        assert_body_contains!(conn, r#""type":"/problem/path-not-found""#);
        assert_headers!(conn, "content-type" => "application/problem+json");
    }

    #[async_std::test]
    async fn handler_root_path() {
        let handler = handler(Some(TEST_JSON));
        let mut conn = get("/test/").on(&handler);
        assert_status!(conn, 404);
        assert_body_contains!(conn, r#""type":"/problem/path-not-found""#);
        assert_headers!(conn, "content-type" => "application/problem+json");
    }

    #[async_std::test]
    async fn handler_string_value() {
        let handler = handler(Some(TEST_JSON));
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
        let handler = handler(Some(TEST_JSON));
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
    async fn handler_array_member() {
        let handler = handler(Some(TEST_JSON));
        assert_response!(
            get("/test/characters/star-trek/0").on(&handler),
            200,
            r#"{"name":"James Kirk","rank":"Captain"}"#,
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
