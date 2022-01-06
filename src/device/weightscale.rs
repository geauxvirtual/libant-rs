use crate::channel::Config;
/// Weightscale device for reading weight and potentially other data from the scale.
// TODO Finish building out this to support all fields that could be returned from the weightscale.
// For instance, my test scale supports sending back more data than just weight, however the
// encoding being use is proprietary compared to the manufacturers newer scale that properly
// supports the ANT+ device pages for a weightscale.
use crate::message::bytes_to_u16;

const WS_DEVICE_TYPE: u8 = 0x77;
const WS_FREQUENCY: u8 = 0x39;
const WS_PERIOD: u16 = 8192;
const WS_TIMEOUT: u8 = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct WeightScale {
    weight: f32, //default in KG
}

impl WeightScale {
    pub fn new() -> Self {
        Self { weight: 0.0 }
    }

    pub fn channel_config() -> Config {
        Config::new()
            .device_type(WS_DEVICE_TYPE)
            .frequency(WS_FREQUENCY)
            .period(WS_PERIOD)
            .timeout(WS_TIMEOUT)
    }

    /// Returns weight in Kilograms
    pub fn weight(&self) -> f32 {
        self.weight
    }

    /// Returns weight in Pounds.
    pub fn weight_in_pounds(&self) -> f32 {
        self.weight * 2.2
    }

    /// Decode broadcast data from the weightscale.
    // TODO: Properly decode the page and other pages that are part of the
    // ANT+ device that can be returned by a weightscale.
    pub fn decode_broadcast_data(&mut self, data: &[u8]) {
        // Check length of slice. Discard for now if not 9.
        if data.len() == 9 {
            match data[1] {
                // First data page includes weight
                0x01 => {
                    if (data[7] != 0xFF || data[7] != 0xFE) && data[8] != 0xFF {
                        self.weight = bytes_to_u16(&data[7..]) as f32 / 100.0;
                    }
                }
                _ => {}
            }
        }
    }
}

// TODO Move testing of config into crate::channel and focus testing in devices
// to decoding of data.
/*#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        let ws = WeightScale::new();
        assert_eq!(ws.channel_type, 0x00);
        assert_eq!(ws.device_id, 0);
        assert_eq!(ws.device_type, 0x77);
        assert_eq!(ws.frequency, 0x39);
        assert_eq!(ws.period, 8192);
        assert_eq!(ws.timeout, 10);
    }

    #[test]
    fn set_device_id() {
        let mut ws = WeightScale::new();
        ws.set_device_id(12345);
        assert_eq!(ws.device_id, 12345);
    }

    #[test]
    fn device_type() {
        let ws = WeightScale::new();
        assert_eq!(ws.device_type(), 0x77);
    }

    #[test]
    fn channel_type() {
        let ws = WeightScale::new();
        assert_eq!(ws.channel_type(), 0x00);
    }
}*/
