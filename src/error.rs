use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("Config error: {0}")]
    ConfigError(#[from] std::io::Error),
    #[error("Parsing error: {0}")]
    ParsingError(#[from] toml::de::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
}
