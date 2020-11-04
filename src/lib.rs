#![allow(dead_code)]
pub mod ant;
mod defines;
pub mod error;
mod message;
mod usb;

pub type Result<T> = std::result::Result<T, error::AntError>;

pub use ant::Ant;
pub use usb::Context;
