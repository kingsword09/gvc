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

impl GvcError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ProjectValidation(_) => "project_validation",
            Self::TomlParsing(_) => "toml_parsing",
            Self::GitOperation(_) => "git_operation",
            Self::Io(_) => "io",
            Self::UserCancelled => "user_cancelled",
        }
    }
}

pub type Result<T> = std::result::Result<T, GvcError>;
