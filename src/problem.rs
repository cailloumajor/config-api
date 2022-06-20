use lazy_static::lazy_static;
use serde::{Serialize, Serializer};
use trillium::{conn_try, Conn, KnownHeaderName, Status};

struct StatusCode(Status);

impl Serialize for StatusCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value: u16 = self.0 as u16;
        serializer.serialize_u16(value)
    }
}

/// Represents problem details as of [RFC7807](https://datatracker.ietf.org/doc/html/rfc7807).
#[derive(Serialize)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    type_uri: String,
    title: String,
    status: StatusCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl ProblemDetails {
    pub fn new(problem_type: &str, title: &str, status: Status) -> Self {
        let mut type_uri = String::from("/problem/");
        type_uri.push_str(problem_type);
        Self {
            type_uri,
            title: title.into(),
            status: StatusCode(status),
            detail: Default::default(),
        }
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

lazy_static! {
    pub static ref PROBLEM_INVALID_CONFIG: ProblemDetails = ProblemDetails::new(
        "config-invalid",
        "static configuration is invalid",
        Status::InternalServerError
    );
}

/// Extension trait that adds methods to [`trillium::Conn`].
pub trait ProblemDetailsConnExt {
    fn with_problem_details(self, details: &ProblemDetails) -> Self;
}

impl ProblemDetailsConnExt for Conn {
    fn with_problem_details(self, details: &ProblemDetails) -> Self {
        let body = conn_try!(serde_json::to_string(details), self);
        self.with_status(details.status.0)
            .with_header(KnownHeaderName::ContentType, "application/problem+json")
            .with_body(body)
            .halt()
    }
}

#[cfg(test)]
mod tests {
    use trillium_testing::prelude::*;

    use super::*;

    async fn handler_without_detail(conn: Conn) -> Conn {
        conn.with_problem_details(&ProblemDetails::new(
            "test-problem",
            "A test problem without detail",
            Status::ImATeapot,
        ))
    }

    async fn handler_with_detail(conn: Conn) -> Conn {
        conn.with_problem_details(
            &ProblemDetails::new("test-problem", "A test problem", Status::ImATeapot)
                .with_detail("Test problem details"),
        )
    }

    #[test]
    fn with_problem_details_without_detail() {
        assert_response!(
            get("/").on(&handler_without_detail),
            Status::ImATeapot,
            r#"{"type":"/problem/test-problem","title":"A test problem without detail","status":418}"#,
            "content-type" => "application/problem+json"
        );
    }
    #[test]
    fn with_problem_details_with_detail() {
        assert_response!(
            get("/").on(&handler_with_detail),
            Status::ImATeapot,
            r#"{"type":"/problem/test-problem","title":"A test problem","status":418,"detail":"Test problem details"}"#,
            "content-type" => "application/problem+json"
        );
    }
}
