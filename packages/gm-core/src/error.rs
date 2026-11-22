use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Config(String),
    NoCurrentGeneration,
    NoPreviousGeneration,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            | Self::Config(message) => write!(formatter, "invalid configuration: {message}"),
            | Self::NoCurrentGeneration => formatter.write_str("no generation is currently active"),
            | Self::NoPreviousGeneration => formatter.write_str("there is no older generation to roll back to"),
        }
    }
}

impl std::error::Error for Error {}
