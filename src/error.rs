use crate::message::Message;
use crossbeam_channel::{SendError, TryRecvError};
use rusb::Error as USBError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AntError {
    #[error("{0}")]
    UsbDeviceError(#[from] USBError),
    #[error("Unable to decode message(s)")]
    UnableToDecode,
    #[error("Unable to send request message")]
    RequestSendError(SendError<Message>),
    #[error("Unable to receive message")]
    MessageTryRecvError(TryRecvError),
    #[error("ANT+ run loop already running")]
    AlreadyRunning,
    #[error("Unable to reset ANT+ USB stick")]
    Reset,
}
