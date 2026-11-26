//! Service metadata and the embedded API reference.

use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;

pub(crate) async fn index() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "payvand-api",
        "routes": crate::router::ROUTES,
    }))
}

pub(crate) async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

/// `GET /api/openapi.json` — the description Swagger UI renders. Written by
/// hand and embedded at compile time: the document is the contract, and it
/// ships with the binary rather than being read from a disk a serverless
/// function does not have.
pub(crate) async fn openapi() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        include_str!("../../openapi.json"),
    )
}

/// `GET /api/docs` — the interactive reference itself.
pub(crate) async fn docs() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../../swagger.html"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use serde_json::Value;

    async fn body(response: impl IntoResponse) -> String {
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        String::from_utf8(bytes.to_vec()).expect("utf8")
    }

    /// The index and the router must list the same paths. One hand-written list
    /// drifts the moment a route is added, so compare it to the source instead.
    #[tokio::test]
    async fn index_lists_exactly_the_router_paths() {
        let response = index().await;
        let listed = response
            .0
            .get("routes")
            .and_then(Value::as_array)
            .expect("routes array")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(listed.len(), crate::router::ROUTES.len());
        for route in crate::router::ROUTES {
            assert!(listed.contains(route), "index is missing {route}");
        }

        let mut from_router = Vec::new();
        let source = include_str!("../router.rs");
        // Skip the doc comment and ROUTES constant: only parse real routes.
        let mut rest = &source[source.find("pub fn build_router").expect("router fn")..];
        while let Some(start) = rest.find(".route(") {
            rest = &rest[start + ".route(".len()..];
            let start = rest.find('"').expect("route quote");
            rest = &rest[start + 1..];
            let end = rest.find('"').expect("route path end");
            from_router.push(&rest[..end]);
            rest = &rest[end + 1..];
        }
        from_router.sort_unstable();
        let mut expected = crate::router::ROUTES.to_vec();
        expected.sort_unstable();
        assert_eq!(from_router, expected, "router paths drifted from ROUTES");
    }

    fn resolve_parameter(spec: &Value, parameter: &Value) -> Value {
        if let Some(reference) = parameter.get("$ref").and_then(Value::as_str) {
            let key = reference.rsplit('/').next().unwrap_or("");
            spec["components"]["parameters"]
                .get(key)
                .cloned()
                .unwrap_or_default()
        } else {
            parameter.clone()
        }
    }

    fn parameter_named(spec: &Value, operation: &Value, name: &str) -> Option<Value> {
        operation
            .get("parameters")?
            .as_array()?
            .iter()
            .map(|parameter| resolve_parameter(spec, parameter))
            .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name))
    }

    #[tokio::test]
    async fn embedded_spec_and_docs_match_the_router() {
        let spec_text = body(openapi().await).await;
        let spec: Value = serde_json::from_str(&spec_text).expect("valid OpenAPI JSON");

        for path in [
            "/",
            "/health",
            "/api/health",
            "/api/docs",
            "/api/openapi.json",
            "/api/admin/payments",
            "/api/admin/waitlist",
            "/api/admin/waitlist/{email}",
        ] {
            assert!(
                spec["paths"].get(path).is_some(),
                "OpenAPI is missing {path}"
            );
        }
        assert!(
            spec["servers"]
                .as_array()
                .expect("servers")
                .iter()
                .any(|server| server["url"] == "http://127.0.0.1:8787"),
            "the local server URL is missing"
        );
        assert!(
            spec["servers"]
                .as_array()
                .expect("servers")
                .iter()
                .any(|server| server["url"] == "https://payvand-api.vercel.app"),
            "the production server URL is missing"
        );

        for (path, operations) in spec["paths"]
            .as_object()
            .expect("paths object")
            .iter()
            .filter(|(path, _)| path.starts_with("/api/admin/"))
        {
            for (method, operation) in
                operations
                    .as_object()
                    .expect("path item")
                    .iter()
                    .filter(|(method, _)| {
                        matches!(method.as_str(), "get" | "post" | "patch" | "delete")
                    })
            {
                let responses = &operation["responses"];
                assert!(
                    responses.get("401").is_some(),
                    "{method} {path} is missing a 401 response"
                );
                assert!(
                    responses.get("403").is_some(),
                    "{method} {path} is missing a 403 response"
                );
            }
        }

        for path in [
            "/api/admin/shops",
            "/api/admin/products",
            "/api/admin/discounts",
        ] {
            let limit = parameter_named(&spec, &spec["paths"][path]["get"], "limit")
                .unwrap_or_else(|| panic!("{path} is missing a limit parameter"));
            assert_eq!(limit["schema"]["default"], 25, "{path} default");
        }

        let html = body(docs().await).await;
        assert!(html.contains("url: '/api/openapi.json'"));
        assert!(html.contains("persistAuthorization: false"));
        assert_eq!(
            html.matches("integrity=\"sha384-").count(),
            2,
            "both CDN assets need an integrity hash"
        );
        assert_eq!(html.matches("crossorigin=\"anonymous\"").count(), 2);
    }
}
