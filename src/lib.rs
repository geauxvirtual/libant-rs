#![allow(dead_code)]
pub mod ant;
mod channel;
mod defines;
pub mod device;
mod error;
pub mod message;
mod usb;

pub type Result<T> = std::result::Result<T, error::AntError>;

pub use ant::{Ant, Request};
pub use crossbeam_channel::unbounded;
pub use usb::Context;

pub use message::combine;
