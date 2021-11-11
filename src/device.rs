/// Device enum for passing in the type of device when opening a channel. As new devices
/// are added to the library, the enum will be extended for each type of device.
pub mod hrm;
pub mod powermeter;
pub mod weightscale;

// Current supported devices
use hrm::HeartRateMonitor;
use weightscale::WeightScale;

use crate::message::{bytes_to_u16, bytes_to_u32};

#[derive(Debug, PartialEq, Clone)]
pub enum Device {
    WeightScale(WeightScale),
    HeartRateMonitor(HeartRateMonitor),
    PowerMeter(powermeter::ChannelConfig),
}

impl Device {
    pub fn device_id(&self) -> u16 {
        match self {
            Device::WeightScale(device) => device.device_id(),
            Device::HeartRateMonitor(device) => device.device_id(),
            Device::PowerMeter(device) => device.device_id(),
        }
    }

    pub fn device_type(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.device_type(),
            Device::HeartRateMonitor(device) => device.device_type(),
            Device::PowerMeter(device) => device.device_type(),
        }
    }

    pub fn channel_type(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.channel_type(),
            Device::HeartRateMonitor(device) => device.channel_type(),
            Device::PowerMeter(device) => device.channel_type(),
        }
    }

    pub fn frequency(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.frequency(),
            Device::HeartRateMonitor(device) => device.frequency(),
            Device::PowerMeter(device) => device.frequency(),
        }
    }

    pub fn period(&self) -> u16 {
        match self {
            Device::WeightScale(device) => device.period(),
            Device::HeartRateMonitor(device) => device.period(),
            Device::PowerMeter(device) => device.period(),
        }
    }

    pub fn timeout(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.timeout(),
            Device::HeartRateMonitor(device) => device.timeout(),
            Device::PowerMeter(device) => device.timeout(),
        }
    }

    pub fn transmission_type(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.transmission_type(),
            Device::HeartRateMonitor(device) => device.transmission_type(),
            Device::PowerMeter(device) => device.transmission_type(),
        }
    }
}

// Common data pages across device types.
// Page 0x50 - Manufacturer Information
pub struct Page0x50([u8; 8]);

impl Page0x50 {
    pub fn hardware_revision(&self) -> u8 {
        self.0[3]
    }

    pub fn manufacturer_id(&self) -> u16 {
        bytes_to_u16(&self.0[4..6])
    }

    pub fn model_number(&self) -> u16 {
        bytes_to_u16(&self.0[6..])
    }
}
// Page 0x51 - Product Information
pub struct Page0x51([u8; 8]);

impl Page0x51 {
    pub fn software_version(&self) -> f32 {
        // Check to see if supplemental version is valid
        if self.0[2] == 0xFF {
            self.0[3] as f32 / 10_f32
        } else {
            (self.0[3] * 100 + self.0[2]) as f32 / 1000_f32
        }
    }

    pub fn serial_number(&self) -> u32 {
        bytes_to_u32(&self.0[4..])
    }
}
// Page 0x52 - Battery Status
pub struct Page0x52([u8; 8]);

impl Page0x52 {
    // Battery Voltage in volts. If Coarse battery voltage equals 0xF, then
    // fractional battery voltage will equal 0xFF as being invalid.
    pub fn battery_voltage(&self) -> Option<f32> {
        // If coarse voltage equals 0x0F, just return None
        if self.coarse_voltage() == 0x0F {
            return None;
        }

        Some(self.coarse_voltage() as f32 + (self.0[6] as f32 / 256_f32))
    }

    // Operating time in hours
    pub fn operating_time(&self) -> f32 {
        (bytes_to_u32(&self.0[3..6]) * self.time_resolution() as u32) as f32 / 3600_f32
    }

    pub fn battery_status(&self) -> &str {
        if self.0[7] & 0x10 == 0x10 {
            return "New";
        }
        if self.0[7] & 0x20 == 0x20 {
            return "Good";
        }
        if self.0[7] & 0x30 == 0x30 {
            return "Ok";
        }
        if self.0[7] & 0x40 == 0x40 {
            return "Low";
        }
        if self.0[7] & 0x50 == 0x50 {
            return "Critical";
        }
        // If our bit matching doesn't match anything, just return "Invalid"
        "Invalid"
    }

    // Time resolution for operating time in seconds.
    fn time_resolution(&self) -> u8 {
        if self.0[7] & 0x80 == 0x80 {
            16
        } else {
            2
        }
    }

    fn coarse_voltage(&self) -> u8 {
        self.0[7] & 0x0F
    }
}
