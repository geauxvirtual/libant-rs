use super::{BatteryStatus, Manufacturer, Page0x50, Page0x51, Page0x52};
use crate::channel::Config;
use crate::message::{bytes_to_u16, AcknowledgeDataMessage};
use std::f32::consts::PI;

// Constant values for PowerMeter channel.
const PM_CHANNEL_TYPE: u8 = 0x00;
const PM_DEVICE_TYPE: u8 = 0x0B;
const PM_FREQUENCY: u8 = 0x39;
const PM_EIGHT_HZ: u16 = 8182;
const PM_FOUR_HZ: u16 = 4091;

// PowerMeter provides a way to decode and use the broadcast data sent from the PowerMeter.
// Page 0x01 -> Calibration Messages
// Page 0x02 -> Get/Set Parameters
// Page 0x03 -> Measurement Output
// Page 0x10 -> Power Only
// Page 0x11 -> Torque at wheel
// Page 0x12 -> Torque at Crank
// Page 0x13 -> Torque Effectiveness/Pedal Smoothness
// Page 0x20 -> Crank/Torque frequency
// Page 0x50 -> Manufacturer Information
// Page 0x51 -> Product Information
// Page 0x52 -> Battery Voltage
// Page 0xE0 -> Right Force Angle
// Page 0xE1 -> Left Force Angle
// Page 0xE2 -> Pedal Position data
#[derive(Debug, Default, Clone)]
pub struct PowerMeter {
    cadence: u8,
    power: u16,
    pedal_power: Option<PedalPower>,
    calibration_value: Option<i16>,
    last_page_0x10: Option<Page0x10>,
    last_page_0x12: Option<Page0x12>,
    page_0x01: Option<Page0x01>,
    page_0x50: Option<Page0x50>,
    page_0x51: Option<Page0x51>,
    page_0x52: Option<Page0x52>,
}

impl PowerMeter {
    pub fn new() -> Self {
        Self {
            calibration_value: None,
            pedal_power: None,
            page_0x01: None,
            page_0x50: None,
            page_0x51: None,
            page_0x52: None,
            last_page_0x10: None,
            last_page_0x12: None,
            ..Default::default()
        }
    }

    pub fn channel_config() -> Config {
        Config::new()
            .device_type(PM_DEVICE_TYPE)
            .frequency(PM_FREQUENCY)
            .period(PM_EIGHT_HZ)
    }
    // Instantaneous cadence from each pages 0x10 and 0x12. If instantaneous cadence
    // isn't set on 0x12, then it's calculated from the data provided.
    pub fn cadence(&self) -> u8 {
        self.cadence
    }

    // Power is calcualted from previous and current 0x10 or 0x12 pages. Instantaneous power
    // on 0x10 is not used.
    pub fn power(&self) -> u16 {
        self.power
    }

    // From page 0x10, power meter can report right power or unknown power. If the field
    // isn't valid, then None is returned. If right is signaled, then a tuple is returned with
    // left/right data. If unknown is set, then we'll just assume it's for right and send
    // back left/right.
    pub fn pedal_power(&self) -> Option<(u8, u8)> {
        if let Some(pedal_power) = &self.pedal_power {
            return Some(pedal_power.distribution());
        }
        None
    }

    pub fn battery_status(&self) -> Option<BatteryStatus> {
        if let Some(page) = &self.page_0x52 {
            return Some(page.battery_status());
        }
        None
    }

    pub fn battery_voltage(&self) -> Option<f32> {
        if let Some(page) = &self.page_0x52 {
            return page.battery_voltage();
        }
        None
    }

    pub fn serial_number(&self) -> Option<u32> {
        if let Some(page) = &self.page_0x51 {
            return Some(page.serial_number());
        }
        None
    }

    pub fn manufacturer(&self) -> Option<Manufacturer> {
        if let Some(page) = &self.page_0x50 {
            return Some(page.manufacturer());
        }
        None
    }

    pub fn calibration_value(&self) -> Option<i16> {
        self.calibration_value
    }

    // TODO Need to properly handle a stop in pedaling. After a PM has been transmitting
    // and cadence stops, the last event page will be sent continously until the next event
    // occurs. This will result in cadence dropping to 0 while event count remains constant.
    pub fn decode(&mut self, data: [u8; 8]) {
        match data[0] {
            0x01 => {
                // We received a calibration page. The calibration page is overloaded and can be
                // one of many. The page will be stored but also store the calibration value
                // because the calibration response page could be overwritten by one of the
                // autozero pages.
                let p = Page0x01(data);
                if p.calibration_value().is_some() {
                    self.calibration_value = p.calibration_value();
                }
                self.page_0x01 = Some(p);
            }
            0x10 => {
                let p = Page0x10(data);
                // If there is a last page, then we can calculate values from current page
                // against last page.
                if let Some(last_page) = &self.last_page_0x10 {
                    // If the current page equals the last page, do nothing.
                    if *last_page == p {
                        return;
                    }
                    let ec_delta = p.event_count().wrapping_sub(last_page.event_count());
                    // If last page is same as current page, do nothing
                    //if ec_delta == 0 && p.cadence() != 0 {
                    //    return;
                    //}
                    let accp_delta = p
                        .accumulated_power()
                        .wrapping_sub(last_page.accumulated_power());
                    if p.cadence() != 0xFF {
                        self.cadence = p.cadence();
                    }
                    self.power = (accp_delta as f32 / ec_delta as f32).round() as u16;
                    if p.pedal_power().is_valid() {
                        self.pedal_power = Some(p.pedal_power());
                    }
                }
                self.last_page_0x10 = Some(p);
            } // Power Only page,
            0x12 => {
                let p = Page0x12(data);
                // If there is a last page, then we can calculate values from current page
                // against last page.
                if let Some(last_page) = &self.last_page_0x12 {
                    // If the current page equals the last page, do nothing.
                    if *last_page == p {
                        return;
                    }
                    // First get deltas from last page to current page
                    let ec_delta = p.event_count().wrapping_sub(last_page.event_count());
                    // If last page event count equals current page event count, just skip.
                    // NEEDS_UPDATING: This was added so as not to process duplicate pages as
                    // this would result in zeroed out power comparing last page to current
                    // page. However, if cadence stops, then the last page is continously sent
                    // until the next event. This results in cadence and power not dropping
                    // to 0 on the display. Should check for cadence being 0, and then setting
                    // power to 0 as well.
                    let cp_delta = p.crank_period().wrapping_sub(last_page.crank_period());
                    let acct_delta = p
                        .accumulated_torque()
                        .wrapping_sub(last_page.accumulated_torque());
                    // If instantaneous cadence is valid and there has only been one event
                    // event_count between pages, set cadence to instantaneous cadence.
                    if p.cadence() != 0xFF && ec_delta == 1 {
                        self.cadence = p.cadence();
                    } else {
                        self.cadence = (60_f32 * (ec_delta as f32 / (cp_delta as f32 / 2048_f32)))
                            .round() as u8;
                    }

                    let angular_velo =
                        (2_f32 * PI * ec_delta as f32) / (cp_delta as f32 / 2048_f32);
                    let avg_torque = acct_delta as f32 / (32_f32 * ec_delta as f32);
                    self.power = (avg_torque * angular_velo).round() as u16;
                }
                self.last_page_0x12 = Some(p);
            } // Torque at Crank page
            0x50 => {
                if self.page_0x50.is_none() {
                    self.page_0x50 = Some(Page0x50(data));
                }
            }
            0x51 => {
                if self.page_0x51.is_none() {
                    self.page_0x51 = Some(Page0x51(data));
                }
            }
            0x52 => self.page_0x52 = Some(Page0x52(data)),
            _ => {} // Do nothing with rest of pages for now.
        }
    }
}

#[derive(Debug, Clone)]
enum PedalPower {
    Right(u8),
    Unknown(u8),
}

impl PedalPower {
    fn is_valid(&self) -> bool {
        !matches!(self, Self::Right(0x7F) | Self::Unknown(0x7F))
    }

    fn distribution(&self) -> (u8, u8) {
        match self {
            Self::Right(value) | Self::Unknown(value) => (100 - value, *value),
        }
    }
}

enum CalibrationMessage {
    Response,
    AutozeroSupport,
    Unknown,
}

enum AutozeroStatus {
    Enabled,
    Disabled,
    Unsupported,
}

enum AutozeroConfig {
    OffEnableNotSupported,
    OffEnableSupported,
    OnEnableNotSupported,
    OnEnableSupported,
}

// Calibration Data Page.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Page0x01([u8; 8]);

impl Page0x01 {
    fn message_type(&self) -> CalibrationMessage {
        match self.0[1] {
            0xAC | 0xAF => CalibrationMessage::Response,
            0x12 => CalibrationMessage::AutozeroSupport,
            _ => CalibrationMessage::Unknown, // Should never see
        }
    }

    fn autozero_status(&self) -> Option<AutozeroStatus> {
        match self.message_type() {
            CalibrationMessage::Response => match self.0[2] {
                0x00 => Some(AutozeroStatus::Disabled),
                0x01 => Some(AutozeroStatus::Enabled),
                0xFF => Some(AutozeroStatus::Unsupported),
                _ => None,
            },
            _ => None,
        }
    }

    fn calibration_value(&self) -> Option<i16> {
        match self.message_type() {
            CalibrationMessage::Response => Some(bytes_to_u16(&self.0[6..]) as i16),
            _ => None,
        }
    }

    fn autozero_configuration(&self) -> Option<AutozeroConfig> {
        match self.message_type() {
            CalibrationMessage::AutozeroSupport => match self.0[2] {
                0x00 => Some(AutozeroConfig::OffEnableNotSupported),
                0x01 => Some(AutozeroConfig::OffEnableSupported),
                0x02 => Some(AutozeroConfig::OnEnableNotSupported),
                0x03 => Some(AutozeroConfig::OnEnableSupported),
                _ => None,
            },
            _ => None,
        }
    }
}

// Standard Power Page
#[derive(Copy, Clone, Debug, PartialEq)]
struct Page0x10([u8; 8]);

impl Page0x10 {
    fn event_count(&self) -> u8 {
        self.0[1]
    }

    fn pedal_power(&self) -> PedalPower {
        let p = self.0[2] & 0x7F;
        if self.0[2] & 0x80 == 0x80 {
            PedalPower::Right(p)
        } else {
            PedalPower::Unknown(p)
        }
    }

    fn cadence(&self) -> u8 {
        self.0[3]
    }

    fn accumulated_power(&self) -> u16 {
        bytes_to_u16(&self.0[4..6])
    }

    fn instantaneous_power(&self) -> u16 {
        bytes_to_u16(&self.0[6..])
    }
}

// Standard Crank Torque Data Page
#[derive(Copy, Clone, Debug, PartialEq)]
struct Page0x12([u8; 8]);

impl Page0x12 {
    fn event_count(&self) -> u8 {
        self.0[1]
    }

    fn crank_ticks(&self) -> u8 {
        self.0[2]
    }

    fn cadence(&self) -> u8 {
        self.0[3]
    }

    fn crank_period(&self) -> u16 {
        bytes_to_u16(&self.0[4..6])
    }

    fn accumulated_torque(&self) -> u16 {
        bytes_to_u16(&self.0[6..])
    }
}

use std::fmt;

impl fmt::Display for Page0x12 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Event Count: {}\nCadence: {}\nCrank Period: {}\n Accumulated Torque: {}",
            self.event_count(),
            self.cadence(),
            self.crank_period(),
            self.accumulated_torque()
        )
    }
}

pub fn manual_calibration(channel: u8) -> AcknowledgeDataMessage {
    AcknowledgeDataMessage::new(channel, &[0x01, 0xAA, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_powermeter_decode_page0x10() {
        let mut pm = PowerMeter::new();
        let page1: [u8; 8] = [0x10, 0xc5, 0xb1, 0x4b, 0x22, 0xc3, 0x57, 0x00];
        pm.decode(page1);
        assert_eq!(pm.cadence, 0);
        assert_eq!(pm.power, 0);
        assert_eq!(pm.last_page_0x10, Some(Page0x10(page1)));
        let page2: [u8; 8] = [0x10, 0xc6, 0xb1, 0x4c, 0x78, 0xce, 0x56, 0x00];
        pm.decode(page2);
        assert_eq!(pm.cadence, page2[3]);
        let to_u16 = |data: &[u8]| (data[0] as u16) + ((data[1] as u16) << 8);
        let power = to_u16(&page2[4..6]).wrapping_sub(to_u16(&page1[4..6]));
        assert_eq!(pm.power, power);
    }

    #[test]
    fn test_powermeter_decode_page0x12() {
        let mut pm = PowerMeter::new();
        let page1: [u8; 8] = [0x12, 0x44, 0x44, 0x4b, 0xce, 0xfe, 0x3d, 0xed];
        pm.decode(page1);
        assert_eq!(pm.cadence, 0);
        assert_eq!(pm.power, 0);
        assert_eq!(pm.last_page_0x12, Some(Page0x12(page1)));
        let page2: [u8; 8] = [0x12, 0x45, 0x45, 0x4b, 0x2c, 0x05, 0x9e, 0xee];
        pm.decode(page2);
        assert_eq!(pm.cadence, page2[3]);
        let to_u16 = |data: &[u8]| (data[0] as u16) + ((data[1] as u16) << 8);
        let ec_delta = page2[1].wrapping_sub(page1[1]);
        let cp_delta = to_u16(&page2[4..6]).wrapping_sub(to_u16(&page1[4..6]));
        let acct_delta = to_u16(&page2[6..]).wrapping_sub(to_u16(&page1[6..]));
        let angular_velo = (2_f32 * PI * ec_delta as f32) / (cp_delta as f32 / 2048_f32);
        let avg_torque = acct_delta as f32 / (32_f32 * ec_delta as f32);
        let power = (avg_torque * angular_velo).round() as u16;
        assert_eq!(pm.power, power);
    }
}
