use super::*;
use crate::auth::accounts::{signin, signup, SigninRequest, SignupRequest};
use crate::auth::test_support::{count, test_state};

fn request(email: &str) -> ForgotPasswordRequest {
    ForgotPasswordRequest {
        email: email.to_string(),
    }
}

#[tokio::test]
async fn forgot_password_answers_the_same_for_every_address() {
    let (state, path) = test_state().await;

    let _ = signup(
        State(state.clone()),
        Json(SignupRequest {
            email: "ada@example.com".into(),
            password: "correct horse battery staple".into(),
        }),
    )
    .await
    .expect("signup");

    let unknown = forgot_password(State(state.clone()), Json(request("nobody@example.com")))
        .await
        .expect("generic answer");
    let known = forgot_password(State(state.clone()), Json(request("ada@example.com")))
        .await
        .expect("generic answer");

    // Identical message either way — no account enumeration.
    assert_eq!(unknown.0.message, known.0.message);
    assert!(unknown.0.ok);
    // Exactly one live link, even after two requests.
    assert_eq!(
        count(&state, "SELECT COUNT(*) FROM password_resets").await,
        1
    );
    // The unknown address created nothing.

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn reset_flow_swaps_the_password_and_revokes_sessions() {
    let (state, path) = test_state().await;

    let signup = signup(
        State(state.clone()),
        Json(SignupRequest {
            email: "ada@example.com".into(),
            password: "correct horse battery staple".into(),
        }),
    )
    .await
    .expect("signup");

    // The real token only ever exists in the email; the test mints one
    // through the same helper the handler uses.
    let connection = state.db.connect().expect("connect");
    let token = create_reset_record(&connection, &signup.0.user.id)
        .await
        .expect("reset record");
    drop(connection);

    let _ = reset_password(
        State(state.clone()),
        Json(ResetPasswordRequest {
            token: token.clone(),
            password: "a brand new password".into(),
        }),
    )
    .await
    .expect("reset accepted");

    // Single use: the record is gone, and so is every session.
    assert_eq!(
        count(&state, "SELECT COUNT(*) FROM password_resets").await,
        0
    );
    assert_eq!(count(&state, "SELECT COUNT(*) FROM sessions").await, 0);

    // Old password refused, new password accepted.
    let old = signin(
        State(state.clone()),
        Json(SigninRequest {
            email: "ada@example.com".into(),
            password: "correct horse battery staple".into(),
        }),
    )
    .await;
    assert!(old.is_err(), "the old password must stop working");

    let fresh = signin(
        State(state.clone()),
        Json(SigninRequest {
            email: "ada@example.com".into(),
            password: "a brand new password".into(),
        }),
    )
    .await;
    assert!(fresh.is_ok(), "the new password must work");

    // Reusing the consumed link fails.
    let reuse = reset_password(
        State(state.clone()),
        Json(ResetPasswordRequest {
            token,
            password: "yet another password".into(),
        }),
    )
    .await;
    assert!(reuse.is_err(), "a used link must not work twice");

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn expired_and_bogus_links_are_rejected() {
    let (state, path) = test_state().await;

    let connection = state.db.connect().expect("connect");
    let hash = hash_password("correct horse battery staple").expect("hash");
    connection
        .execute(
            "INSERT INTO users (id, email, password_hash, wallet, created_at) \
             VALUES ('user-1', 'ada@example.com', ?, NULL, 1)",
            [hash],
        )
        .await
        .expect("insert");
    let token = create_reset_record(&connection, "user-1")
        .await
        .expect("reset record");
    // Age the link past its expiry.
    connection
        .execute("UPDATE password_resets SET expires_at = 1", ())
        .await
        .expect("age");
    drop(connection);

    let expired = reset_password(
        State(state.clone()),
        Json(ResetPasswordRequest {
            token,
            password: "a brand new password".into(),
        }),
    )
    .await;
    assert!(expired.is_err(), "an expired link must not work");

    let bogus = reset_password(
        State(state.clone()),
        Json(ResetPasswordRequest {
            token: "not-a-real-token".into(),
            password: "a brand new password".into(),
        }),
    )
    .await;
    assert!(bogus.is_err(), "a bogus link must not work");

    let _ = std::fs::remove_file(path);
}
