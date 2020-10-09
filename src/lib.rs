pub mod ant;
mod device;
pub mod error;

pub type Result<T> = std::result::Result<T, error::AntError>;
