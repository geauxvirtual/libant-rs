#[derive(Clone, Debug, PartialEq)]
pub struct PowerMeter {
    channel_type: u8,
    device_id: u16,
    device_type: u8,
    frequency: u8,
    period: u16,
    timeout: u8,
    transmission_type: u8,
}

impl PowerMeter {
    pub fn new() -> Self {
        PowerMeter {
            channel_type: 0x00,
            device_id: 0,
            device_type: 0x0B,
            frequency: 0x39,
            period: 8192,
            timeout: 10,
            transmission_type: 0, //set to 0 for pairing if unknown
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
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        let pm = PowerMeter::new();
        assert_eq!(pm.channel_type, 0x00);
        assert_eq!(pm.device_id, 0);
        assert_eq!(pm.device_type, 0x0B);
        assert_eq!(pm.frequency, 0x39);
        assert_eq!(pm.period, 8192);
        assert_eq!(pm.timeout, 10);
    }

    #[test]
    fn set_device_id() {
        let mut pm = PowerMeter::new();
        pm.set_device_id(12345);
        assert_eq!(pm.device_id, 12345);
    }
}
