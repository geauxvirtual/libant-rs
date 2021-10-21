/// Device enum for passing in the type of device when opening a channel. As new devices
/// are added to the library, the enum will be extended for each type of device.
pub mod hrm;
pub mod weightscale;

// Current supported devices
use hrm::HeartRateMonitor;
use weightscale::WeightScale;

#[derive(Debug, PartialEq, Clone)]
pub enum Device {
    WeightScale(WeightScale),
    HeartRateMonitor(HeartRateMonitor),
}

impl Device {
    pub fn device_id(&self) -> u16 {
        match self {
            Device::WeightScale(device) => device.device_id(),
            Device::HeartRateMonitor(device) => device.device_id(),
        }
    }

    pub fn device_type(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.device_type(),
            Device::HeartRateMonitor(device) => device.device_type(),
        }
    }

    pub fn channel_type(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.channel_type(),
            Device::HeartRateMonitor(device) => device.channel_type(),
        }
    }

    pub fn frequency(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.frequency(),
            Device::HeartRateMonitor(device) => device.frequency(),
        }
    }

    pub fn period(&self) -> u16 {
        match self {
            Device::WeightScale(device) => device.period(),
            Device::HeartRateMonitor(device) => device.period(),
        }
    }

    pub fn timeout(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.timeout(),
            Device::HeartRateMonitor(device) => device.timeout(),
        }
    }

    pub fn transmission_type(&self) -> u8 {
        match self {
            Device::WeightScale(device) => device.transmission_type(),
            Device::HeartRateMonitor(device) => device.transmission_type(),
        }
    }
}
