use serde::Serialize;

/// Every command returns this error type so the UI gets a stable shape to
/// show: a machine-readable `kind` and a human-readable `message`.
///
/// Messages must never include chat content. Library errors used here
/// (SQLite, JSON, zip, IO) report positions and codes, not data.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("The vault is locked. Enter your password to continue.")]
    Locked,

    #[error("A vault already exists. Unlock it instead of creating a new one.")]
    VaultExists,

    #[error("No vault exists yet. Create one first.")]
    NoVault,

    #[error("That password is not correct.")]
    WrongPassword,

    #[error("The password is accepted but the database cannot be read. It may be damaged: {0}")]
    Corrupt(String),

    #[error(
        "This database was written by a newer version of the app (schema {found}, this version \
         understands up to {supported}). Install the newer version to open it. Nothing has been changed."
    )]
    NewerDatabase { found: i64, supported: i64 },

    #[error("{0}")]
    InvalidInput(String),

    #[error("An import is already running.")]
    Busy,

    #[error("The import was cancelled. Nothing was changed.")]
    Cancelled,

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("File error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Could not read the export archive: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Could not parse JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Could not use the keychain: {0}")]
    Keychain(String),

    #[error("No Anthropic API key is set. Add one in Settings.")]
    NoApiKey,

    #[error("The Anthropic API returned an error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Could not reach the Anthropic API: {0}")]
    Network(String),

    #[error("The model's answer could not be used: {0}")]
    BadModelOutput(String),

    #[error("Key derivation failed: {0}")]
    Kdf(String),

    #[error("Application error: {0}")]
    Tauri(#[from] tauri::Error),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::Locked => "locked",
            AppError::VaultExists => "vault_exists",
            AppError::NoVault => "no_vault",
            AppError::WrongPassword => "wrong_password",
            AppError::Corrupt(_) => "corrupt",
            AppError::NewerDatabase { .. } => "newer_database",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::Busy => "busy",
            AppError::Cancelled => "cancelled",
            AppError::Database(_) => "database",
            AppError::Io(_) => "io",
            AppError::Zip(_) => "zip",
            AppError::Json(_) => "json",
            AppError::Keychain(_) => "keychain",
            AppError::NoApiKey => "no_api_key",
            AppError::Api { .. } => "api",
            AppError::Network(_) => "network",
            AppError::BadModelOutput(_) => "bad_model_output",
            AppError::Kdf(_) => "kdf",
            AppError::Tauri(_) => "tauri",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
