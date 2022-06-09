use anyhow::{ensure, Context};
use async_std::fs;
use serde_json::Value as JSONValue;
use tide::{Request, Response, StatusCode};
use toml::Value as TOMLValue;

use crate::AppState;

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

pub async fn load_config(path: &str) -> anyhow::Result<TOMLValue> {
    let content = fs::read_to_string(path)
        .await
        .context("error reading configuration file")?;
    ensure!(!content.is_empty(), "configuration file is empty");
    Ok(content.parse()?)
}

pub async fn handler(req: Request<AppState>) -> tide::Result {
    let mut subset = &req.state().toml_value.read().await.clone();
    let path = req.param("path").unwrap();
    for subpath in path.split('/') {
        let got = subset.get(subpath);
        if let Some(value) = got {
            subset = value;
        } else {
            return Ok(Response::builder(StatusCode::NotFound)
                .body("Path was not found in configuration")
                .into());
        }
    }
    Ok(toml_to_json(subset).into())
}

#[cfg(test)]
mod tests {
    use async_std::sync::{Arc, RwLock};
    use std::io::Write;
    use std::path::Path;
    use tempfile::NamedTempFile;
    use test_case::test_case;
    use tide::http::mime;
    use tide_testing::TideTestingExt;

    use crate::AppState;

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

    #[async_std::test]
    async fn handler() {
        let toml_value = Arc::new(RwLock::new(toml::from_str(TEST_TOML).unwrap()));
        let mut app = tide::with_state(AppState { toml_value });
        app.at("/test/*path").get(super::handler);

        let mut resp = app.get("/test/nonexistent").await.unwrap();
        assert_eq!(resp.status(), 404);
        assert_eq!(
            resp.body_string().await.unwrap(),
            "Path was not found in configuration"
        );

        // let mut resp = app.get("/test/").await.unwrap();
        // assert_eq!(resp.status(), 200);
        // assert!(resp
        //     .header("Content-Type")
        //     .unwrap()
        //     .contains(&mime::JSON.into()));
        // let body_json: serde_json::Value = resp.body_json().await.unwrap();
        // assert!(!body_json.as_object().unwrap().is_empty());
        // TODO: replace below test case with the one above when tide matches
        //       empty wildcard (v0.17.0).
        let resp = app.get("/test/").await.unwrap();
        assert_eq!(resp.status(), 404);

        let mut resp = app.get("/test/title").await.unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp
            .header("Content-Type")
            .unwrap()
            .contains(&mime::JSON.into()));
        assert_eq!(resp.body_string().await.unwrap(), r#""TOML example 😊""#);

        let mut resp = app.get("/test/servers/alpha").await.unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp
            .header("Content-Type")
            .unwrap()
            .contains(&mime::JSON.into()));
        assert_eq!(
            resp.body_string().await.unwrap(),
            r#"{"enabled":false,"hostname":"server1","ip":"10.0.0.1"}"#
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
