use thiserror::Error;

#[derive(Error, Debug)]
pub enum GvcError {
    #[error("Project validation failed: {0}")]
    ProjectValidation(String),

    #[error("TOML parsing failed: {0}")]
    TomlParsing(String),

    #[error("Git operation failed: {0}")]
    GitOperation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Operation cancelled by user")]
    UserCancelled,
}

pub type Result<T> = std::result::Result<T, GvcError>;
