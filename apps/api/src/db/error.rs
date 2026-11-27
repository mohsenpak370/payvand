//! The one error type opening and migrating a database can fail with.

use std::fmt;

#[derive(Debug)]
pub struct DbError(String);

impl DbError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DbError {}

impl From<libsql::Error> for DbError {
    fn from(error: libsql::Error) -> Self {
        Self(error.to_string())
    }
}
