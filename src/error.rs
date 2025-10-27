use thiserror::Error;

#[derive(Error, Debug)]
pub enum GvcError {
    #[error("Project validation failed: {0}")]
    ProjectValidation(String),

    #[allow(dead_code)]
    #[error("Gradle execution failed: {0}")]
    GradleExecution(String),

    #[error("TOML parsing failed: {0}")]
    TomlParsing(String),

    #[allow(dead_code)]
    #[error("Git operation failed: {0}")]
    GitOperation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Operation cancelled by user")]
    UserCancelled,
}

pub type Result<T> = std::result::Result<T, GvcError>;
