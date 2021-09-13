#[derive(Clone, Debug, PartialEq)]
pub struct WeightScale {
    channel_type: u8,
    device_id: u16,
    device_type: u8,
    frequency: u8,
    period: u16,
    timeout: u8,
    transmission_type: u8,
    weight: f32, //default in KG
}

impl WeightScale {
    pub fn new() -> Self {
        WeightScale {
            channel_type: 0x00,
            device_id: 0,
            device_type: 0x77,
            frequency: 0x39,
            period: 8192,
            timeout: 10,
            transmission_type: 0, //set to 0 for pairing if unknown
            weight: 0.0,
        }
    }

    pub fn set_device_id(&mut self, device_id: u16) -> &mut Self {
        self.device_id = device_id;
        self
    }

    pub fn set_transmission_type(&mut self, transmission_type: u8) -> &mut Self {
        self.transmission_type = transmission_type;
        self
    }

    pub fn device_id(&self) -> u16 {
        self.device_id
    }

    pub fn channel_type(&self) -> u8 {
        self.channel_type
    }

    pub fn device_type(&self) -> u8 {
        self.device_type
    }

    pub fn frequency(&self) -> u8 {
        self.frequency
    }

    pub fn period(&self) -> u16 {
        self.period
    }

    pub fn timeout(&self) -> u8 {
        self.timeout
    }

    pub fn transmission_type(&self) -> u8 {
        self.transmission_type
    }

    // Returns weight in Kilograms
    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn weight_in_pounds(&self) -> f32 {
        self.weight * 2.2
    }

    pub fn decode_broadcast_data(&mut self, data: &[u8]) {
        // Check length of slice. Discard for now if not 9.
        if data.len() == 9 {
            match data[1] {
                // First data page includes weight
                0x01 => {
                    if (data[7] != 0xFF || data[7] != 0xFE) && data[8] != 0xFF {
                        self.weight = crate::combine(&data[7..]) as f32 / 100.0;
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
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
}