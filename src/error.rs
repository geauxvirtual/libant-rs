use thiserror::Error;
use rusb::Error as USBError;

#[derive(Error, Debug)]
pub enum AntError {
    #[error("{0}")]
    UsbDeviceError(#[from] USBError)
}

