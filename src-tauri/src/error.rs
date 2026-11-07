use serde::Serialize;

/// Every command returns this error type so the UI gets a stable shape to
/// show: a machine-readable `kind` and a human-readable `message`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("application error: {0}")]
    Tauri(#[from] tauri::Error),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let kind = match self {
            AppError::Tauri(_) => "tauri",
        };
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", kind)?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
