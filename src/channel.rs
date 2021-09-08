use crate::device::Device;
use crate::message::{self, ChannelResponseMessage, Message};

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

#[derive(Debug, PartialEq, Clone)]
pub struct Channel {
    state: State,
    number: u8,
    device: Device,
}

impl Channel {
    pub fn new(number: u8, device: Device) -> Self {
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

    pub fn assign(&self, network: u8) -> Message {
        message::assign_channel(self.number, self.device.channel_type(), network)
    }

    // Probably not needed
    //pub fn unassign(&self) -> Message {
    //    message::unassign_channel(self.number)
    //}

    pub fn set_channel_id(&self) -> Message {
        message::set_channel_id(
            self.number,
            self.device.device_id(),
            self.device.device_type(),
            self.device.transmission_type(),
        )
    }

    pub fn set_hp_search_timeout(&self) -> Message {
        message::set_hp_search_timeout(self.number, self.device.timeout())
    }

    pub fn set_period(&self) -> Message {
        message::set_channel_period(self.number, self.device.period())
    }

    pub fn set_frequency(&self) -> Message {
        message::set_channel_frequency(self.number, self.device.frequency())
    }

    pub fn open(&self) -> Message {
        message::open_channel(self.number)
    }

    // Probably not needed
    //pub fn close(&self) -> Message {
    //    message::close_channel(self.number)
    //}
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        let device = Device::WeightScale(crate::device::weightscale::WeightScale::new());
        let channel = Channel::new(0, device);
        assert_eq!(channel.state, State::Assign);
        assert_eq!(channel.number, 0);
        assert_eq!(
            channel.device,
            Device::WeightScale(crate::device::weightscale::WeightScale::new())
        );
    }
}
