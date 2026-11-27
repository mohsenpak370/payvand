use super::*;
use crate::auth::credentials::{hash_password, verify_password};

#[tokio::test]
async fn migrates_and_reads_users_back() {
    let (_db, connection) = crate::db::in_memory().await.expect("in-memory schema");

    let hash = hash_password("correct horse battery staple").expect("hash");
    connection
        .execute(
            "INSERT INTO users (id, email, password_hash, wallet, created_at) \
             VALUES ('user-1', 'ada@example.com', ?, NULL, 1)",
            [hash],
        )
        .await
        .expect("insert");

    let (user, stored) = read_user_with_hash(&connection, "ada@example.com")
        .await
        .expect("query")
        .expect("user exists");
    assert_eq!(user.id, "user-1");
    assert_eq!(user.wallet, None);
    assert!(verify_password("correct horse battery staple", &stored));
    assert!(read_user_with_hash(&connection, "missing@example.com")
        .await
        .expect("query")
        .is_none());
}
