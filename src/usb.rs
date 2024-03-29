/// A UsbContext and UsbDevice for interacting with the physical
/// USB device.
use std::time::Duration;

pub use rusb::{Context, UsbContext};
use rusb::{DeviceHandle, Error};

use super::{error::AntError, Result};

// TODO ANT settings are currently hardcoded and work with the test
// USB device, but need to verify if these settings work with other
// ANT+ USB devices.
const VENDOR_ID: u16 = 0x0FCF;
const USB_ANT_CONFIGURATION: u8 = 1;
const USB_ANT_INTERFACE: u8 = 0;
const USB_ANT_EP_IN: u8 = 0x81;
const USB_ANT_EP_OUT: u8 = 0x01;

/// UsbDevice struct that holds the device handle to the USB device
/// along with a buffer to read data data from.
pub struct UsbDevice<T: UsbContext> {
    handle: DeviceHandle<T>,
}

impl<T: UsbContext> UsbDevice<T> {
    /// Initialize the USB device for the ANT+ device plugged in.
    pub fn init(ctx: &mut T) -> Result<UsbDevice<T>> {
        for device in ctx.devices()?.iter() {
            let device_desc = device.device_descriptor()?;
            if device_desc.vendor_id() == VENDOR_ID {
                let mut handle = device.open()?;
                match handle.reset() {
                    Ok(_) => {
                        handle.claim_interface(USB_ANT_INTERFACE)?;
                    }
                    Err(Error::NotFound) => {
                        let mut handle = device.open()?;
                        handle.claim_interface(USB_ANT_INTERFACE)?;
                    }
                    Err(e) => return Err(AntError::UsbDeviceError(e)),
                }
                return Ok(UsbDevice { handle });
            }
        }
        Err(AntError::UsbDeviceError(Error::NoDevice))
    }

    /// Read from the USB device with a timeout of 10 milliseconds.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.read_with_timeout(buf, Duration::from_millis(10))
    }

    /// Read from the USB device with the specified timeout.
    pub fn read_with_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        self.handle
            .read_bulk(USB_ANT_EP_IN, buf, timeout)
            .map_err(AntError::UsbDeviceError)
    }

    /// Write message to the USB device with a timeout of 1 second.
    pub fn write(&self, message: &[u8]) -> Result<usize> {
        self.write_with_timeout(message, Duration::from_secs(1))
    }

    /// Write message to the USB device with a specified timeout.
    pub fn write_with_timeout(&self, message: &[u8], timeout: Duration) -> Result<usize> {
        self.handle
            .write_bulk(USB_ANT_EP_OUT, message, timeout)
            .map_err(AntError::UsbDeviceError)
    }
}
