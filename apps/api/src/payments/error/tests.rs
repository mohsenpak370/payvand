use super::*;

/// An internal failure names a table, a query or a file. The caller gets a
/// sentence instead; the detail goes to the log.
#[tokio::test]
async fn an_internal_error_keeps_its_detail_to_the_log() {
    use axum::response::IntoResponse;
    let response = ApiError::Internal("no such table: secret_ledger".into()).into_response();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .expect("body");
    let text = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(!text.contains("secret_ledger"), "leaked: {text}");
    assert!(text.contains("Something went wrong"));
}
