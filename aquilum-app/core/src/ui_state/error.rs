use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "code", content = "details", rename_all = "snake_case")]
pub enum UiStateError {
    Database { message: String },
    Io { message: String },
    Task { message: String },
    InvalidInput { message: String },
    UnsupportedSchema { version: i64 },
    Unavailable { message: String },
}

impl From<rusqlite::Error> for UiStateError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database {
            message: error.to_string(),
        }
    }
}

impl From<std::io::Error> for UiStateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            message: error.to_string(),
        }
    }
}

impl From<crate::app_core::TaskFailed> for UiStateError {
    fn from(error: crate::app_core::TaskFailed) -> Self {
        Self::Task {
            message: error.to_string(),
        }
    }
}
