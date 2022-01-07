/// A channel is a means of communication for an ANT+ device. Typcially an ANT+ USB device can
/// support up to a certain number of channels on each network that it supports. A channel
/// gets mapped to a single device. Even if multiple devices are sending data, the first device
/// learned by the channel will have its data routed through the configured channel. If multiple
/// devices of the same type are to be used, multiple channels need to be opened.
use crate::message::{self, ChannelResponseMessage, Message};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    device_id: u16,
    device_type: u8,
    channel_type: u8,
    frequency: u8,
    period: u16,
    timeout: u8,
    transmission_type: u8,
}

impl Config {
    pub fn new() -> Self {
        Config {
            channel_type: 0x00,
            timeout: 30,
            ..Default::default()
        }
    }

    pub fn device_id(mut self, device_id: u16) -> Self {
        self.device_id = device_id;
        self
    }

    pub fn device_type(mut self, device_type: u8) -> Self {
        self.device_type = device_type;
        self
    }

    pub fn channel_type(mut self, channel_type: u8) -> Self {
        self.channel_type = channel_type;
        self
    }

    pub fn frequency(mut self, frequency: u8) -> Self {
        self.frequency = frequency;
        self
    }

    pub fn period(mut self, period: u16) -> Self {
        self.period = period;
        self
    }

    pub fn timeout(mut self, timeout: u8) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn transmission_type(mut self, transmission_type: u8) -> Self {
        self.transmission_type = transmission_type;
        self
    }
}

#[derive(Debug, PartialEq, Clone)]
enum State {
    Assign,
    Unassign,
    SetDeviceId,
    SetTimeout,
    SetFrequency,
    SetPeriod,
    Open,
    Closed,
    Ready,
}

/// Channel maintains the channel number, state of the channel, and the device
/// for the channel configuration parameters.
#[derive(Debug, PartialEq, Clone)]
pub struct Channel {
    state: State,
    number: u8,
    device: Config,
}

impl Channel {
    pub fn new(number: u8, device: Config) -> Self {
        Channel {
            state: State::Assign,
            number,
            device,
        }
    }

    pub fn number(&self) -> u8 {
        self.number
    }

    // TODO: Happy path for now, we only route messages that are
    // ReponseNoError. We'll just check to verify the message received
    // is what we expect in the current state, then transition the state or
    // log the error.
    /// Routes messages for the channel when opening the channel for the specified
    /// device type.
    pub fn route(&mut self, mesg: &ChannelResponseMessage) -> Option<Message> {
        match self.state {
            State::Assign => {
                if mesg.message_id() == message::MESG_ASSIGN_CHANNEL_ID {
                    log::debug!(
                        "Setting channel state to SetDeviceId. Sending set_channel_id message"
                    );
                    self.state = State::SetDeviceId;
                    return Some(self.set_channel_id());
                }
                // Need to handle if there is an error when setting up the channel
                // Doing an option for now, but may never return None.
                None
            }
            State::SetDeviceId => {
                if mesg.message_id() == message::MESG_CHANNEL_ID_ID {
                    log::debug!("Setting channel state to SetTimeout. Sending set_timeout message");
                    self.state = State::SetTimeout;
                    return Some(self.set_hp_search_timeout());
                }
                None
            }
            State::SetTimeout => {
                if mesg.message_id() == message::MESG_CHANNEL_SEARCH_TIMEOUT_ID {
                    log::debug!("Setting channel state to SetPeriod. Sending set_period message");
                    self.state = State::SetPeriod;
                    return Some(self.set_period());
                }
                None
            }
            State::SetPeriod => {
                if mesg.message_id() == message::MESG_CHANNEL_MESG_PERIOD_ID {
                    log::debug!(
                        "Setting channel state to SetFrequency. Sending set_frequency message"
                    );
                    self.state = State::SetFrequency;
                    return Some(self.set_frequency());
                }
                None
            }
            State::SetFrequency => {
                if mesg.message_id() == message::MESG_CHANNEL_RADIO_FREQ_ID {
                    log::debug!("Setting channel state to Open. Sending open_channel message");
                    self.state = State::Open;
                    return Some(self.open());
                }
                None
            }
            State::Open => {
                if mesg.message_id() == message::MESG_OPEN_CHANNEL_ID {
                    log::info!("Channel {:?} is open", self.number);
                    return None;
                }
                None
            }
            _ => {
                log::debug!("Unsupported channel message in current state: {:x?}", mesg);
                unimplemented!()
            }
        }
    }

    /// Assigns a channel to the specified network.
    pub fn assign(&self, network: u8) -> Message {
        message::assign_channel(self.number, self.device.channel_type, network)
    }

    /// Sets the channel id based on the device parameters.
    pub fn set_channel_id(&self) -> Message {
        message::set_channel_id(
            self.number,
            self.device.device_id,
            self.device.device_type,
            self.device.transmission_type,
        )
    }

    /// Sets the search timeout.
    pub fn set_hp_search_timeout(&self) -> Message {
        message::set_hp_search_timeout(self.number, self.device.timeout)
    }

    /// Sets the period for the channel for how often a message is expected.
    pub fn set_period(&self) -> Message {
        message::set_channel_period(self.number, self.device.period)
    }

    /// Sets the frequency for the device
    pub fn set_frequency(&self) -> Message {
        message::set_channel_frequency(self.number, self.device.frequency)
    }

    /// Open the channel to start receiving broadcast data from the device.
    pub fn open(&self) -> Message {
        message::open_channel(self.number)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        let config = Config::new();
        let channel = Channel::new(0, config);
        assert_eq!(channel.state, State::Assign);
        assert_eq!(channel.number, 0);
        assert_eq!(channel.device, Config::new());
    }
}
