//! Email + password accounts with database-backed sessions.
//!
//! Sessions are opaque 32 byte random tokens returned in the response body;
//! the client sends them back as `Authorization: Bearer <token>`. Only the
//! SHA-256 digest of a token is stored, so a leaked database dump cannot be
//! replayed as a live session.
//!
//! `users.wallet` is intentionally nullable: it is the slot for linking a
//! Solana address later so the same account can also sign in with a wallet.
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `auth/` directory, one file (plus its tests) per responsibility:
//!
//! - `accounts`: the signup, signin, signout and `me` handlers
//! - `recovery`: forgot-password and reset-password
//! - `session`: issuing, resolving and requiring bearer sessions
//! - `users`: the `User` row and its queries
//! - `credentials`: email normalisation and password hashing
//! - `support`: ids, clocks and error helpers shared across the crate

mod accounts;
mod credentials;
mod recovery;
mod session;
mod support;
mod users;

#[cfg(test)]
mod test_support;

pub use accounts::{me, signin, signout, signup, AuthResponse, SigninRequest, SignupRequest};
pub use credentials::{hash_password, verify_password, MIN_PASSWORD_LEN};
pub use recovery::{
    forgot_password, reset_password, ForgotPasswordRequest, ResetPasswordRequest,
    ResetSentResponse, RESET_TTL_SECS,
};
pub use session::{optional_user, require_user, SESSION_TTL_SECS};
pub use users::User;

pub(crate) use credentials::normalize_email;
pub(crate) use session::issue_session;
pub(crate) use support::{internal, is_unique_violation, nullable_text, random_id, unix_now};
pub(crate) use users::{read_user, seller_account};
