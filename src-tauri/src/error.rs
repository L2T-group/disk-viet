use serde::{Serialize, Serializer};

/// Error type returned by every command. Serialized to the frontend as
/// `{ kind: string, message: string }`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("blocked by safety guard: {0}")]
    Guard(String),
    #[error("nothing has been scanned yet")]
    NotScanned,
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Invalid(_) => "invalid",
            AppError::Cancelled => "cancelled",
            AppError::Guard(_) => "guard",
            AppError::NotScanned => "notScanned",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
