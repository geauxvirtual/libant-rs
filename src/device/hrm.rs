use super::{BatteryStatus, Manufacturer};
use crate::channel::Config;
/// Heartrate Monitor device. Each data page contains HR data. Legacy devices
/// only have a data page 0. Newer devices have multiple pages with a MSB bit
/// flip every four pages to signify legacy or newer device.
// TODO: Support get capabilities and changing mode of HR device if device
// supports wimming or running data.
use crate::message::{bytes_to_u16, bytes_to_u32, AcknowledgeDataMessage};

const HRM_DEVICE_TYPE: u8 = 0x78;
const HRM_FREQUENCY: u8 = 0x39;
const HRM_PERIOD: u16 = 8070;
const HRM_TIMEOUT: u8 = 10;
const COMMON_DATA_PAGE_70: u8 = 0x46;

// TODO Split out channel config from device broadcast data
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeartRateMonitor {
    heartrate: u8,
    last_heartbeat_event: f32,
    heartbeat_count: u8,
    operating_time: u32,
    manufacturer_id: u8,
    serial_number: u16,
    hardware_version: u8,
    software_version: u8,
    model_number: u8,
    battery_level: u8,
    fractional_battery_voltage: u8,
    descriptive_bit_field: u8,
}

impl HeartRateMonitor {
    pub fn new() -> Self {
        HeartRateMonitor {
            ..Default::default()
        }
    }

    pub fn channel_config() -> Config {
        Config::new()
            .device_type(HRM_DEVICE_TYPE)
            .frequency(HRM_FREQUENCY)
            .period(HRM_PERIOD)
            .timeout(HRM_TIMEOUT)
    }

    /// Decoded heartrate received from broadcast data
    pub fn heartrate(&self) -> u8 {
        self.heartrate
    }

    /// Manufacturer of the hardware device
    /// HRM devices use a u8 field while other devices use u16.
    pub fn manufacturer(&self) -> Manufacturer {
        Manufacturer::from(self.manufacturer_id as u16)
    }

    /// Serial number of device, typically the ANT+ ID
    pub fn serial_number(&self) -> u16 {
        self.serial_number
    }

    /// Hardware version of the device
    pub fn hardware_version(&self) -> u8 {
        self.hardware_version
    }

    /// Software version of the devices
    pub fn software_version(&self) -> u8 {
        self.software_version
    }

    /// Model number of the device
    pub fn model_number(&self) -> u8 {
        self.model_number
    }

    /// Whole number 0 - 100 as percentage. Set to 0xFF if not used.
    pub fn battery_level(&self) -> u8 {
        self.battery_level
    }

    /// Fractional battery voltage provided by the device
    pub fn fractional_battery_voltage(&self) -> f32 {
        self.fractional_battery_voltage as f32 / 256_f32
    }

    /// Coarse battery voltage provided by the device
    pub fn coarse_battery_voltage(&self) -> u8 {
        self.descriptive_bit_field & 0x0F
    }

    /// Battery status as a str from the data provided by the device.
    pub fn battery_status(&self) -> BatteryStatus {
        BatteryStatus::from(self.descriptive_bit_field)
    }

    /// Decode broadcast data received from ANT+ device.
    /// Every heartrate broadcast data page includes heartrate data.
    pub fn decode_broadcast_data(&mut self, data: &[u8]) {
        // Check length of slice. Discard for now if not 9.
        if data.len() == 8 {
            match data[0] {
                // Data page 0 Default or unknown data page (legacy)
                0x00 | 0x80 => {}
                // Data page 1 Cumulative Operating Time
                0x01 | 0x81 => self.operating_time = bytes_to_u32(&data[1..4]),
                // Data page 2 Manufacturer Information
                0x02 | 0x82 => {
                    self.manufacturer_id = data[1];
                    self.serial_number = bytes_to_u16(&data[2..4]);
                }
                // Data page 3 Product Information
                0x03 | 0x83 => {
                    self.hardware_version = data[1];
                    self.software_version = data[2];
                    self.model_number = data[3];
                }
                // Data page 4 Previous Heart Beat
                0x04 | 0x84 => {}
                // Data page 5 Swim Interval Summary
                0x05 | 0x85 => {}
                // Data page 6 Capabilities
                0x06 | 0x86 => {}
                // Data page 7 Battery Status
                0x07 | 0x87 => {
                    self.battery_level = data[1];
                    self.fractional_battery_voltage = data[2];
                    self.descriptive_bit_field = data[3];
                }
                _ => return, //Drop message if none of these pages
            }
            self.last_heartbeat_event = bytes_to_u16(&data[4..6]) as f32 / 1000_f32;
            self.heartbeat_count = data[6];
            self.heartrate = data[7];
        }
    }

    /// Sends an Acknowledge data page to the heart rate monitor requesting
    /// the manufacturer information.
    pub fn request_manufacturer_info(&self, channel_number: u8) -> AcknowledgeDataMessage {
        AcknowledgeDataMessage::new(channel_number, &self.request_data_page(0x02))
    }

    /// Send an Acknowledge data page to the heart rate monitor requesting
    /// the battery status for the heart rate monitor.
    pub fn request_battery_status(&self, channel_number: u8) -> AcknowledgeDataMessage {
        AcknowledgeDataMessage::new(channel_number, &self.request_data_page(0x07))
    }

    /// The general acknowledge data page to send to the heart rate monitor
    /// requesting a specific page to be sent back to the device.
    fn request_data_page(&self, page_number: u8) -> [u8; 8] {
        [
            COMMON_DATA_PAGE_70,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0x01,
            page_number,
            0x01,
        ]
    }
}

// TODO Move this testing under crate::channel and focus device testing on decoding
// of data.
/*#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        let hrm = HeartRateMonitor::new();
        assert_eq!(hrm.channel_type, 0x00);
        assert_eq!(hrm.device_id, 0);
        assert_eq!(hrm.device_type, 0x78);
        assert_eq!(hrm.frequency, 0x39);
        assert_eq!(hrm.period, 8070);
        assert_eq!(hrm.timeout, 10);
    }

    #[test]
    fn set_device_id() {
        let mut hrm = HeartRateMonitor::new();
        hrm.set_device_id(12345);
        assert_eq!(hrm.device_id, 12345);
    }

    #[test]
    fn device_type() {
        let hrm = HeartRateMonitor::new();
        assert_eq!(hrm.device_type(), 0x78);
    }

    #[test]
    fn channel_type() {
        let hrm = HeartRateMonitor::new();
        assert_eq!(hrm.channel_type(), 0x00);
    }
}*/
