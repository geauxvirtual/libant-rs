use crate::device::Device;
use rusb::{Context, UsbContext};

use super::Result;

pub struct Ant<T: UsbContext> {
    device: Device<T>,
}

impl<T: UsbContext> Ant<T> {
    pub fn init() -> Result<Ant<T>> {
        //let mut ctx = Context::new()?;
        Ok(Ant {
            device: Device::init()?,
        })
    }
}
