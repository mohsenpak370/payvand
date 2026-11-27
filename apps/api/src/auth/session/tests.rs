use super::*;

#[test]
fn session_tokens_are_stored_hashed() {
    let token = random_id();
    assert_ne!(token_hash(&token), token);
    assert_eq!(token_hash(&token), token_hash(&token));
    assert_ne!(random_id(), random_id());
}
