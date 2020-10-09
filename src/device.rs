use rusb::{Context, DeviceHandle, UsbContext};

use super::Result;

const VENDOR_ID: u16 = 0x0FCF;
const USB_ANT_CONFIGURATION: u8 = 1;
const USB_ANT_INTERFACE: u8 = 0;

pub struct Device<T: UsbContext> {
    handle: DeviceHandle<T>,
}

impl<T: UsbContext> Device<T> {
    pub fn init() -> Result<Device<T>> {
        for mut device in rusb::DeviceList::new()?.iter() {
            let device_desc = device.device_descriptor()?;
            if device_desc.vendor_id() == VENDOR_ID {
                let mut handle = device.open()?;
                match handle.reset() {
                    Ok(_) => {
                        handle.claim_interface(USB_ANT_INTERFACE)?;
                        return Ok(Device { handle });
                    }
                    Err(_) => unimplemented!(),
                }
            }
        }
        unimplemented!();
    }
}
