//! Service metadata and the embedded API reference.

use std::collections::HashSet;
use std::sync::OnceLock;

use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::Value;

const FULL_OPENAPI: &str = include_str!("../../openapi.json");
static PUBLIC_OPENAPI: OnceLock<String> = OnceLock::new();

pub(crate) async fn index() -> Json<Value> {
    Json(serde_json::json!({
        "service": "payvand-api",
        "routes": crate::router::public_routes(),
    }))
}

pub(crate) async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

/// Collect schema names referenced from a part of the document. Called on the
/// public paths and shared components, never on `components.schemas` itself —
/// otherwise admin schemas would keep one another alive.
fn collect_schema_refs(value: &Value, refs: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                    refs.insert(name.to_string());
                }
            }
            for child in map.values() {
                collect_schema_refs(child, refs);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_schema_refs(child, refs);
            }
        }
        _ => {}
    }
}

/// The public view of the embedded specification. The canonical document in
/// the repository stays complete — including admin operations and their
/// 401/403 responses — but the API host advertises only the public contract.
/// Admin tools live in the web app's `/admin` page.
fn public_openapi() -> &'static str {
    PUBLIC_OPENAPI.get_or_init(|| {
        let mut spec: Value =
            serde_json::from_str(FULL_OPENAPI).expect("embedded OpenAPI must be valid JSON");

        if let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) {
            paths.retain(|path, _| !path.starts_with("/api/admin/"));
        }
        if let Some(tags) = spec.get_mut("tags").and_then(Value::as_array_mut) {
            tags.retain(|tag| tag.get("name").and_then(Value::as_str) != Some("admin"));
        }
        if let Some(info) = spec.get_mut("info") {
            info["description"] = Value::String(
                "Accounts, Solana payment invoices, shops, discounts and wallet links for Payvand.\n\nEvery route below lives on the API origin. Routes marked with a session need `Authorization: Bearer <token>` from `/api/auth/signin` or `/api/auth/signup`. Admin operations are available in the web app's `/admin` page and are not part of this public reference."
                    .to_string(),
            );
        }

        // Schemas used only by admin operations would otherwise still appear
        // in Swagger's schema list. Keep exactly what the public contract uses.
        let mut referenced = HashSet::new();
        if let Some(paths) = spec.get("paths") {
            collect_schema_refs(paths, &mut referenced);
        }
        if let Some(components) = spec.get("components").and_then(Value::as_object) {
            for (name, component) in components {
                if name != "schemas" {
                    collect_schema_refs(component, &mut referenced);
                }
            }
        }
        if let Some(schemas) = spec
            .get_mut("components")
            .and_then(|components| components.get_mut("schemas"))
            .and_then(Value::as_object_mut)
        {
            schemas.retain(|name, _| referenced.contains(name));
        }

        serde_json::to_string(&spec).expect("filtered OpenAPI must serialize")
    })
}

/// `GET /api/openapi.json` — the description Swagger UI renders. Written by
/// hand and embedded at compile time: the document is the contract, and it
/// ships with the binary rather than being read from a disk a serverless
/// function does not have.
pub(crate) async fn openapi() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        public_openapi(),
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

    async fn body(response: impl IntoResponse) -> String {
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        String::from_utf8(bytes.to_vec()).expect("utf8")
    }

    /// The index advertises the public contract: every public route is listed,
    /// and none of the internal admin routes leak into it.
    #[tokio::test]
    async fn index_lists_public_routes_only() {
        let response = index().await;
        let listed = response
            .0
            .get("routes")
            .and_then(Value::as_array)
            .expect("routes array")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        let public = crate::router::public_routes();
        assert_eq!(listed.len(), public.len());
        for route in public {
            assert!(listed.contains(&route), "index is missing {route}");
        }
        assert!(
            listed.iter().all(|route| !route.starts_with("/api/admin/")),
            "index advertised an admin route"
        );
    }

    /// The router and its hand-written path list must not drift.
    #[tokio::test]
    async fn router_matches_declared_routes() {
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
        let full: Value = serde_json::from_str(FULL_OPENAPI).expect("valid canonical OpenAPI JSON");
        let public: Value =
            serde_json::from_str(&body(openapi().await).await).expect("valid public OpenAPI JSON");

        for path in [
            "/",
            "/health",
            "/api/health",
            "/api/docs",
            "/api/openapi.json",
        ] {
            assert!(
                public["paths"].get(path).is_some(),
                "public OpenAPI is missing {path}"
            );
        }
        for path in [
            "/api/admin/payments",
            "/api/admin/waitlist",
            "/api/admin/waitlist/{email}",
        ] {
            assert!(
                full["paths"].get(path).is_some(),
                "canonical OpenAPI is missing {path}"
            );
        }
        assert!(
            public["servers"]
                .as_array()
                .expect("servers")
                .iter()
                .any(|server| server["url"] == "http://127.0.0.1:8787"),
            "the local server URL is missing"
        );
        assert!(
            public["servers"]
                .as_array()
                .expect("servers")
                .iter()
                .any(|server| server["url"] == "https://payvand-api.vercel.app"),
            "the production server URL is missing"
        );

        // The canonical document keeps the auth contract for admin operations,
        // even though the public view does not advertise them.
        for (path, operations) in full["paths"]
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
            let limit = parameter_named(&full, &full["paths"][path]["get"], "limit")
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

    /// Swagger must show only the public surface. The admin endpoints stay in
    /// the router and in the canonical spec; they are simply not advertised.
    #[tokio::test]
    async fn public_spec_hides_admin_routes_and_tag() {
        let public: Value =
            serde_json::from_str(&body(openapi().await).await).expect("valid public OpenAPI JSON");

        assert!(
            public["paths"]
                .as_object()
                .expect("paths object")
                .keys()
                .all(|path| !path.starts_with("/api/admin/")),
            "public OpenAPI advertised an admin route"
        );
        assert!(
            !public["tags"]
                .as_array()
                .expect("tags")
                .iter()
                .any(|tag| tag["name"] == "admin"),
            "public OpenAPI advertised the admin tag"
        );

        let schemas = public["components"]["schemas"]
            .as_object()
            .expect("schemas");
        assert!(
            !schemas.keys().any(|name| name.starts_with("Admin")),
            "public OpenAPI advertised an admin schema"
        );
        for name in ["PaymentsPage", "WaitlistEntry", "WaitlistPage"] {
            assert!(
                !schemas.contains_key(name),
                "public OpenAPI advertised admin-only schema {name}"
            );
        }

        let description = public["info"]["description"].as_str().expect("description");
        assert!(description.contains("/admin"));
        assert!(!description.contains("ADMIN_EMAILS"));
    }
}
