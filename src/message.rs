// TODO Go through this and figure out legacy code from new code.
/// Message module provides a way for creating messages to send to the ANT+
/// USB device or ANT+ device along with providing a way to decode messages
/// received from the ANT+ USB device or ANT+ device sending data on a channel.
use crate::defines;
use log::debug;
use std::convert::TryInto;
use std::fmt;

pub const MESG_TX_SYNC: u8 = 0xA4;
pub const MESG_RX_SYNC: u8 = 0xA5;
pub const MESG_SYNC_SIZE: usize = 1;
pub const MESG_SIZE_SIZE: usize = 1;
pub const MESG_ID_SIZE: usize = 1;
pub const MESG_CHANNEL_NUM_SIZE: usize = 1;
pub const MESG_EXT_MESG_BF_SIZE: usize = 1;
pub const MESG_CHECKSUM_SIZE: usize = 1;
pub const MESG_DATA_SIZE: usize = 9;

pub const MESG_ANT_MAX_PAYLOAD_SIZE: usize = defines::ANT_STANDARD_DATA_PAYLOAD_SIZE;
pub const MESG_MAX_EXT_DATA_SIZE: usize =
    defines::ANT_EXT_MESG_DEVICE_ID_FIELD_SIZE + defines::ANT_EXT_STRING_SIZE;
pub const MESG_MAX_DATA_SIZE: usize =
    MESG_ANT_MAX_PAYLOAD_SIZE + MESG_EXT_MESG_BF_SIZE + MESG_MAX_EXT_DATA_SIZE;
pub const MESG_MAX_SIZE_VALUE: usize = MESG_MAX_DATA_SIZE + MESG_CHANNEL_NUM_SIZE;
pub const MESG_BUFFER_SIZE: usize =
    MESG_SIZE_SIZE + MESG_ID_SIZE + MESG_CHANNEL_NUM_SIZE + MESG_MAX_DATA_SIZE + MESG_CHECKSUM_SIZE;
pub const MESG_FRAMED_SIZE: usize = MESG_ID_SIZE + MESG_CHANNEL_NUM_SIZE + MESG_MAX_DATA_SIZE;
pub const MESG_HEADER_SIZE: usize = MESG_SYNC_SIZE + MESG_SIZE_SIZE + MESG_ID_SIZE;
pub const MESG_FRAME_SIZE: usize = MESG_HEADER_SIZE + MESG_CHECKSUM_SIZE;
pub const MESG_MAX_SIZE: usize = MESG_MAX_DATA_SIZE + MESG_FRAME_SIZE;
pub const MESG_SIZE_OFFSET: usize = MESG_SYNC_SIZE;
pub const MESG_ID_OFFSET: usize = MESG_SYNC_SIZE + MESG_SIZE_SIZE;
pub const MESG_DATA_OFFSET: usize = MESG_HEADER_SIZE;
pub const MESG_RECOMMENDED_BUFFER_SIZE: u8 = 64;

pub const RESPONSE_NO_ERROR: u8 = 0x00;
pub const MESG_EVENT_ID: u8 = 0x01;
pub const MESG_RESPONSE_EVENT_ID: u8 = 0x40;
pub const MESG_UNASSIGN_CHANNEL_ID: u8 = 0x41;
pub const MESG_ASSIGN_CHANNEL_ID: u8 = 0x42;
pub const MESG_CHANNEL_MESG_PERIOD_ID: u8 = 0x43;
pub const MESG_CHANNEL_SEARCH_TIMEOUT_ID: u8 = 0x44;
pub const MESG_CHANNEL_RADIO_FREQ_ID: u8 = 0x45;
pub const MESG_NETWORK_KEY_ID: u8 = 0x46;
pub const MESG_RESET: u8 = 0x4A;
pub const MESG_OPEN_CHANNEL_ID: u8 = 0x4B;
pub const MESG_CLOSE_CHANNEL_ID: u8 = 0x4C;
pub const MESG_REQUEST: u8 = 0x4D;
pub const MESG_BROADCAST_DATA_ID: u8 = 0x4E;
pub const MESG_ACKNOWLEDGE_DATA_ID: u8 = 0x4F;
pub const MESG_CHANNEL_ID_ID: u8 = 0x51;
pub const MESG_CAPABILITIES_ID: u8 = 0x54;
pub const MESG_STARTUP_MESG_ID: u8 = 0x6F;
pub const MESG_CREATE_CHANNEL_ID: u8 = 0xFE;
// Not part of ANT+ standard. Using as control message for quitting
const MESG_QUIT: u8 = 0xFF;

pub const EVENT_RX_SEARCH_TIMEOUT: u8 = 0x01;
pub const EVENT_CHANNEL_CLOSED: u8 = 0x07;
pub const CHANNEL_IN_WRONG_STATE: u8 = 0x15;

/// ReadBuffer provides a buffer to through data received from the ANT+ USB device and turn
/// the data into a Message
pub struct ReadBuffer {
    index: usize,
    inner: Vec<u8>,
}

impl ReadBuffer {
    pub fn new(buffer: &[u8]) -> Self {
        ReadBuffer {
            index: 0,
            inner: buffer.to_vec(),
        }
    }
}

// This is an iterator over the read in buffer from the ANT+ USB stick.
// The buffer is a variable size [u8] that we will loop through looking
// for a sync bit and then creating an ANT message from the received
// data. If the checksum of a message is invalid, then we continue searching
// for another sync bit. If we find no sync bits or if checksums are invalid,
// we do not create messages and none would be returned.
impl Iterator for ReadBuffer {
    // Use this for now until switch to enum
    type Item = Response;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index >= self.inner.len() {
                return None;
            }
            if self.inner[self.index] == MESG_TX_SYNC {
                let mut checksum = 0;
                let index = self.index;
                // Length of message
                let len = index + self.inner[index + 1] as usize + 4;
                // Verify checksum
                for i in index..len {
                    checksum ^= self.inner[i];
                }
                // Set self.index to current message length
                if checksum == 0 {
                    self.index = len;
                    return Some(process_message(&self.inner[index..len - 1]));
                }
            }
            self.index += 1;
        }
    }
}

/// Responses that can be received from the ANT+ USB device.
/// Startup are messages received when initially configuring the USB device.
/// ChannelResponse are messages received from the channel during configuration of the channel or
/// events received while running.
/// BroadcastData is data received from ANT+ device.
#[derive(Debug, PartialEq)]
pub enum Response {
    Startup(StartupMessage),
    ChannelResponse(ChannelResponseMessage),
    BroadcastData(BroadcastDataMessage),
}

#[derive(Debug, PartialEq)]
pub struct StartupMessage(u8);

impl StartupMessage {
    fn reason(&self) -> StartupReason {
        match self.0 {
            0x00 => StartupReason::PowerOnReset,
            0x01 => StartupReason::HardwareResetLine,
            0x02 => StartupReason::WatchDogReset,
            0x20 => StartupReason::CommandReset,
            0x40 => StartupReason::SynchronousReset,
            0x80 => StartupReason::SuspendReset,
            _ => StartupReason::Error,
        }
    }
}

// Maybe write our own debug here
#[derive(Debug, PartialEq)]
pub enum StartupReason {
    PowerOnReset,
    HardwareResetLine,
    WatchDogReset,
    CommandReset,
    SynchronousReset,
    SuspendReset,
    Error,
}

#[derive(Debug, PartialEq)]
pub enum ChannelResponseCode {
    ResponseNoError,
    EventRxSearchTimeout,
    EventRxFail,
    EventTx,
    EventTransferTxCompleted,
    EventTransferTxFailed,
    EventChannelClosed,
    EventRxFailGoToSearch,
    ChannelCollision,
    ChannelInWrongState,
}
// TODO: May need to increase the size of this if support for encryption for devices
// is added, but not needed right now.
#[derive(Debug, PartialEq)]
pub struct ChannelResponseMessage([u8; 3]);

impl ChannelResponseMessage {
    pub fn from(mesg: &[u8]) -> Self {
        Self(mesg.try_into().expect("Wrong number of elements passed"))
    }

    pub fn channel(&self) -> u8 {
        self.0[0]
    }

    pub fn message_id(&self) -> u8 {
        self.0[1]
    }

    pub fn code(&self) -> ChannelResponseCode {
        match self.0[2] {
            0x00 => ChannelResponseCode::ResponseNoError,
            0x01 => ChannelResponseCode::EventRxSearchTimeout,
            0x02 => ChannelResponseCode::EventRxFail,
            0x03 => ChannelResponseCode::EventTx,
            0x05 => ChannelResponseCode::EventTransferTxCompleted,
            0x06 => ChannelResponseCode::EventTransferTxFailed,
            0x07 => ChannelResponseCode::EventChannelClosed,
            0x08 => ChannelResponseCode::EventRxFailGoToSearch,
            0x09 => ChannelResponseCode::ChannelCollision,
            0x15 => ChannelResponseCode::ChannelInWrongState,
            _ => {
                debug!("Received ChannelResponseCode: {:x}", self.0[2]);
                unimplemented!();
            }
        }
    }
}

// TODO: See if this impacts extended messages. Most likely does.
#[derive(Clone, Debug, PartialEq)]
pub struct BroadcastDataMessage([u8; 9]);

impl BroadcastDataMessage {
    // TODO: Should this return an error if user tries to pass in
    // data longer than 8?
    pub fn new(channel_number: u8, data: &[u8]) -> Self {
        let mut buf: [u8; 9] = [0; 9];
        buf[0] = channel_number;
        buf[1..].copy_from_slice(data);
        Self(buf)
    }

    pub fn from(mesg: &[u8]) -> Self {
        Self(mesg.try_into().expect("Wrong number of elements passed"))
    }

    pub fn channel(&self) -> u8 {
        self.0[0]
    }

    pub fn data(&self) -> &[u8] {
        &self.0[1..]
    }

    pub fn to_message(&self) -> Message {
        Message::new(MESG_BROADCAST_DATA_ID, &self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AcknowledgeDataMessage([u8; 9]);

impl AcknowledgeDataMessage {
    // TODO: Should this return an error if user tries to pass in
    // data longer than 8?
    pub fn new(channel_number: u8, data: &[u8]) -> Self {
        let mut buf: [u8; 9] = [0; 9];
        buf[0] = channel_number;
        buf[1..].copy_from_slice(data);
        Self(buf)
    }

    pub fn from(mesg: &[u8]) -> Self {
        Self(mesg.try_into().expect("Wrong number of elements passed"))
    }

    pub fn channel(&self) -> u8 {
        self.0[0]
    }

    pub fn data(&self) -> &[u8] {
        &self.0[1..]
    }

    pub fn to_message(&self) -> Message {
        Message::new(MESG_ACKNOWLEDGE_DATA_ID, &self.0)
    }
}

#[derive(Clone, PartialEq)]
pub struct Message {
    pub id: u8,
    pub data: Vec<u8>,
}

impl Message {
    pub fn new(id: u8, data: &[u8]) -> Message {
        // TODO: Validate size of messgae is < MESG_MAX_DATA_SIZE
        Message {
            id,
            data: data.to_vec(),
        }
    }

    // Converts a message into something that can be written out
    pub fn encode(&self) -> Vec<u8> {
        let size = self.data.len();
        let total_size = MESG_HEADER_SIZE + size;
        let mut buf: Vec<u8> = vec![0; total_size + 3];
        buf[0] = MESG_TX_SYNC;
        buf[MESG_SIZE_OFFSET] = size as u8;
        buf[MESG_ID_OFFSET] = self.id;
        buf[MESG_DATA_OFFSET..total_size].copy_from_slice(&self.data);

        let mut checksum = 0;
        // Calulcate checksum
        for i in 0..total_size {
            checksum ^= buf[i];
        }
        buf[total_size] = checksum;
        buf
    }

    fn id_as_str(&self) -> &'static str {
        match self.id {
            MESG_STARTUP_MESG_ID => "Startup (0x6F)",
            MESG_CAPABILITIES_ID => "Capabilities (0x54)",
            MESG_RESPONSE_EVENT_ID => "Response Event (0x40)",
            MESG_BROADCAST_DATA_ID => "Broadcast Data (0x4E)",
            MESG_CHANNEL_ID_ID => "Channel ID Request (0x51)",
            _ => "Unknown message",
        }
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Message ID: {} ", self.id_as_str())?;
        write!(f, "DATA: ")?;
        match self.id {
            MESG_STARTUP_MESG_ID => write!(f, "{}", startup_reason(self.data[0])),
            MESG_CAPABILITIES_ID => write!(
                f,
                "Max ANT+ Channels: {} MAX ANT+ Networks: {}",
                self.data[0], self.data[1]
            ),
            MESG_RESPONSE_EVENT_ID => {
                if self.data[1] != MESG_NETWORK_KEY_ID {
                    write!(f, "Channel: {} ", self.data[0])?;
                }
                match self.data[1] {
                    MESG_NETWORK_KEY_ID => {
                        write!(f, "Network: {} ", self.data[0])?;
                        match self.data[1] & self.data[2] {
                            RESPONSE_NO_ERROR => write!(f, "Response: Network key set"),
                            _ => {
                                write!(f, "Error: {:X} received setting network key", self.data[2])
                            }
                        }
                    }
                    MESG_ASSIGN_CHANNEL_ID => match self.data[1] & self.data[2] {
                        RESPONSE_NO_ERROR => write!(f, "Response: Assigned"),
                        _ => write!(
                            f,
                            "Error: {:X} received trying to assign channel",
                            self.data[2]
                        ),
                    },
                    MESG_CHANNEL_ID_ID => match self.data[1] & self.data[2] {
                        RESPONSE_NO_ERROR => write!(f, "Response: Set device id"),
                        _ => write!(
                            f,
                            "Error: {:X} received trying to set device id",
                            self.data[2]
                        ),
                    },
                    MESG_UNASSIGN_CHANNEL_ID => match self.data[1] & self.data[2] {
                        RESPONSE_NO_ERROR => write!(f, "Response: Unassigned"),
                        _ => match self.data[2] {
                            CHANNEL_IN_WRONG_STATE => write!(
                                f,
                                "Error: channel in wrong state received trying to unassign channel"
                            ),
                            _ => write!(
                                f,
                                "Error: {:X} received trying to unassign channel",
                                self.data[2]
                            ),
                        },
                    },
                    MESG_CHANNEL_SEARCH_TIMEOUT_ID => match self.data[1] & self.data[2] {
                        RESPONSE_NO_ERROR => {
                            write!(f, "Response: High priority search timeout configured")
                        }
                        _ => write!(
                            f,
                            "Error: {:X} received trying to configure high priority search",
                            self.data[2]
                        ),
                    },
                    MESG_CHANNEL_MESG_PERIOD_ID => match self.data[1] & self.data[2] {
                        RESPONSE_NO_ERROR => write!(f, "Response: Message period configured"),
                        _ => write!(
                            f,
                            "Error: {:X} received trying to configure message period",
                            self.data[2]
                        ),
                    },
                    MESG_CHANNEL_RADIO_FREQ_ID => match self.data[1] & self.data[2] {
                        RESPONSE_NO_ERROR => write!(f, "Response: Radio frequency configured"),
                        _ => write!(
                            f,
                            "Error: {:X} received trying to configure radio frequency",
                            self.data[2]
                        ),
                    },
                    MESG_OPEN_CHANNEL_ID => match self.data[1] & self.data[2] {
                        RESPONSE_NO_ERROR => write!(f, "Response: Channel opened"),
                        _ => write!(
                            f,
                            "Error: {:X} received trying to open channel",
                            self.data[2]
                        ),
                    },
                    MESG_CLOSE_CHANNEL_ID => match self.data[1] & self.data[2] {
                        RESPONSE_NO_ERROR => write!(f, "Response: Channel closed"),
                        _ => write!(
                            f,
                            "Error: {:X} received trying to close channel",
                            self.data[2]
                        ),
                    },
                    MESG_EVENT_ID => match self.data[2] {
                        EVENT_RX_SEARCH_TIMEOUT => write!(f, "Event: Channel search timed out"),
                        EVENT_CHANNEL_CLOSED => write!(f, "Event: Channel closed"),
                        _ => write!(f, "Unknown event: {:X}", self.data[2]),
                    },
                    _ => write!(f, "Unknown response event: {:X}", self.data[1]),
                }
            }
            MESG_BROADCAST_DATA_ID => write!(f, "Broadcast Data Received: {:?}", self.data),
            MESG_CHANNEL_ID_ID => write!(
                f,
                "Channel: {:X} Device Number: {:?} Device Type: {:?}",
                self.data[0],
                bytes_to_u16(&self.data[1..3]),
                device_type(self.data[3])
            ),
            _ => write!(f, "{:?}", self.data),
        }
    }
}

fn startup_reason(data: u8) -> &'static str {
    if data == 0x00 {
        return "Reason: Power On Reset";
    };
    if data & 0x20 == 0x20 {
        return "Reason: Reset from command";
    };
    if data & 0x80 == 0x80 {
        return "Reason: Suspend";
    };
    if data & 0x01 == 0x01 {
        return "Reason: Reset";
    };
    if data & 0x02 == 0x02 {
        return "Reason: WDT";
    };
    if data & 0x40 == 0x40 {
        return "Reason: Sync";
    };
    "unknown reason"
}

/// Process message takes a slice of bytes received in the ReadBuffer and converts the data into
/// the correct Response
fn process_message(buf: &[u8]) -> Response {
    match buf[MESG_ID_OFFSET] {
        MESG_STARTUP_MESG_ID => Response::Startup(StartupMessage(buf[MESG_DATA_OFFSET])),
        MESG_RESPONSE_EVENT_ID => {
            Response::ChannelResponse(ChannelResponseMessage::from(&buf[MESG_DATA_OFFSET..]))
        }
        MESG_BROADCAST_DATA_ID => {
            Response::BroadcastData(BroadcastDataMessage::from(&buf[MESG_DATA_OFFSET..]))
        }
        _ => {
            println!("Mesg: {:x?}", buf);
            unimplemented!();
        }
    }
}

pub fn reset() -> Message {
    Message::new(MESG_RESET, &[0; 15])
}

pub fn set_network_key(network_number: u8, key: &[u8]) -> Message {
    let mut data = vec![network_number];
    data.extend(key);
    Message::new(MESG_NETWORK_KEY_ID, &data)
}

pub fn get_capabilities() -> Message {
    Message::new(MESG_REQUEST, &[0, MESG_CAPABILITIES_ID])
}

pub fn get_channel_id(channel: u8) -> Message {
    Message::new(MESG_REQUEST, &[channel, MESG_CHANNEL_ID_ID])
}

pub fn assign_channel(channel: u8, channel_type: u8, network: u8) -> Message {
    Message::new(MESG_ASSIGN_CHANNEL_ID, &[channel, channel_type, network])
}

pub fn set_channel_id(
    channel: u8,
    device_id: u16,
    device_type: u8,
    transmission_type: u8,
) -> Message {
    Message::new(
        MESG_CHANNEL_ID_ID,
        &[
            channel,
            (device_id & 0xFF) as u8,
            ((device_id >> 8) & 0xFF) as u8,
            device_type,
            transmission_type,
        ],
    )
}

pub fn set_hp_search_timeout(channel: u8, timeout: u8) -> Message {
    Message::new(MESG_CHANNEL_SEARCH_TIMEOUT_ID, &[channel, timeout])
}

pub fn set_channel_period(channel: u8, period: u16) -> Message {
    Message::new(
        MESG_CHANNEL_MESG_PERIOD_ID,
        &[channel, (period & 0xFF) as u8, ((period >> 8) & 0xFF) as u8],
    )
}

pub fn set_channel_frequency(channel: u8, frequency: u8) -> Message {
    Message::new(MESG_CHANNEL_RADIO_FREQ_ID, &[channel, frequency])
}

pub fn open_channel(channel: u8) -> Message {
    Message::new(MESG_OPEN_CHANNEL_ID, &[channel])
}

pub fn close_channel(channel: u8) -> Message {
    Message::new(MESG_CLOSE_CHANNEL_ID, &[channel])
}

pub fn unassign_channel(channel: u8) -> Message {
    Message::new(MESG_UNASSIGN_CHANNEL_ID, &[channel])
}

// Using this for now to simply creating a channel for now. May change later
// Profiles: HR 0x00, Power: 0x01
pub fn create_channel(channel: u8, device_id: u16, profile: u8) -> Message {
    Message::new(
        MESG_CREATE_CHANNEL_ID,
        &[
            channel,
            (device_id & 0xFF) as u8,
            ((device_id >> 8) & 0xFF) as u8,
            profile,
        ],
    )
}

// App message to quit our threads for now
pub fn quit() -> Message {
    Message::new(MESG_QUIT, &[0])
}

// This is for debugging purposes only right now as we build the app
pub fn print(m: &Message) {
    match m.id {
        MESG_STARTUP_MESG_ID => {
            println!("Message id: MESG_STARTUP_MESG_ID");
            startup_reason(m.data[0]);
        }
        MESG_CAPABILITIES_ID => {
            println!("Capabilities");
            println!("--Max Ant Channels: {:?}", m.data[0]);
            println!("--Max Ant Networks: {:?}", m.data[1]);
        }
        MESG_RESPONSE_EVENT_ID => {
            println!("Message id: MESG_RESPONSE_EVENT_ID");
            match m.data[1] {
                MESG_NETWORK_KEY_ID => {
                    if m.data[2] != RESPONSE_NO_ERROR {
                        println!("Response: Error setting network key");
                    }
                    println!("Response: Network key set");
                }
                MESG_ASSIGN_CHANNEL_ID => match m.data[2] {
                    RESPONSE_NO_ERROR => println!("Response: Channel assigned"),
                    _ => println!("Response: Error assigning channel"),
                },
                MESG_CHANNEL_ID_ID => match m.data[2] {
                    RESPONSE_NO_ERROR => println!("Response: Channel id assigned"),
                    _ => println!("Response: Error assigning channel id"),
                },
                MESG_UNASSIGN_CHANNEL_ID => match m.data[2] {
                    RESPONSE_NO_ERROR => println!("Response: Channel unassigned"),
                    _ => println!("Response: Error unassigning channel - {:X}", m.data[2]),
                },
                MESG_CHANNEL_SEARCH_TIMEOUT_ID => match m.data[2] {
                    RESPONSE_NO_ERROR => println!("Response: High priority search disabled"),
                    _ => println!("Response: Error disabling high priority search"),
                },
                MESG_CHANNEL_MESG_PERIOD_ID => match m.data[2] {
                    RESPONSE_NO_ERROR => println!("Response: Channel period set"),
                    _ => println!("Response: Error setting channel period"),
                },
                MESG_CHANNEL_RADIO_FREQ_ID => match m.data[2] {
                    RESPONSE_NO_ERROR => println!("Response: Channel frequency set"),
                    _ => println!("Response: Error setting channel frequency"),
                },
                MESG_OPEN_CHANNEL_ID => match m.data[2] {
                    RESPONSE_NO_ERROR => println!("Response: Channel open"),
                    _ => println!("Response: Error opening channel"),
                },
                MESG_CLOSE_CHANNEL_ID => match m.data[2] {
                    RESPONSE_NO_ERROR => println!("Response: Channel closed"),
                    _ => println!("Response: Error closing channel - {:X}", m.data[2]),
                },
                _ => println!("We don't know this response yet: {:?}", m),
            }
        }
        MESG_BROADCAST_DATA_ID => {
            print!("Channel: {:X} ", m.data[0]);
            match m.data[0] {
                0 => {
                    match m.data[1] {
                        4 | 132 => {
                            print!("Data Page: Previous heart beat, ");
                            print!(
                                "Previous heatbeat time: {:?}, ",
                                bytes_to_u16(&m.data[3..5])
                            );
                        }
                        2 | 130 => {
                            print!("Data Page: Manufacturer Information, ");
                            print!("Manufacturer: {:?}, ", get_manufacturer(m.data[2]));
                            print!("Upper serial: {:X}, ", bytes_to_u16(&m.data[3..5]));
                        }
                        3 | 131 => {
                            print!("Data Page: Product Information, ");
                            print!("Hardware version: {:?}, ", m.data[2]);
                            print!("Software version: {:?}, ", m.data[3]);
                            print!("Model Number: {:?}, ", m.data[4]);
                        }
                        1 | 129 => {
                            print!("Data Page: Operating Time, ");
                            print!("Operating Time: {:?}, ", bytes_to_u32(&m.data[2..5]));
                        }
                        _ => print!("Unknown data page: {:?}, ", m.data[1]),
                    }
                    print!("Heartbeat event time: {:?}, ", bytes_to_u16(&m.data[5..7]));
                    print!("Heatbeat count: {:?}, ", m.data[7]);
                    println!("Heartbeat: {:?}", m.data[8]);
                }
                1 => match m.data[1] {
                    0x10 => {
                        print!("Cadence: {:?}, ", m.data[4]);
                        println!("Power: {:?}", bytes_to_u16(&m.data[7..]));
                    }
                    0x12 => {
                        print!("Events: {:?}, ", m.data[2]);
                        print!("Ticks: {:?}, ", m.data[3]);
                        print!("Cadence: {:?}, ", m.data[4]);
                        print!("Period: {:?}, ", bytes_to_u16(&m.data[5..7]));
                        println!("Torque: {:?}", bytes_to_u16(&m.data[7..]));
                    }
                    _ => println!("We don't know this page yet: {:X}", m.data[1]),
                },
                _ => println!("We don't know this channel yet: {:?}", m.data[0]),
            }
        }
        MESG_CHANNEL_ID_ID => {
            print!("Channel: {:X}, ", m.data[0]);
            print!("Device Number: {:?}, ", bytes_to_u16(&m.data[1..3]));
            println!("Device Type: {:?}", device_type(m.data[3]));
        }
        _ => println!("We don't know this message yet: {:X}", m.id),
    }
}

fn get_manufacturer(id: u8) -> &'static str {
    match id {
        32 => "Wahoo",
        _ => "Unknown",
    }
}

fn device_type(device_type: u8) -> &'static str {
    match device_type {
        0x78 => "Heartrate monitor",
        _ => "unknown device type",
    }
}

// bytes_to_u16 takes a byte slice formatted in [LSB, MSB] and combines the two fields together
// into a single u16.
pub(crate) fn bytes_to_u16(b: &[u8]) -> u16 {
    if b.len() > 2 {
        log::error!("Slice larger than 2. Returning just first two bytes combined");
    }
    match b.len() {
        1 => b[0] as u16,
        _ => (b[0] as u16) + ((b[1] as u16) << 8),
    }
}

// bytes_to_u32 takes a byte slice formatted in
// [LSB, MSB], [LSB, DATA ,MSB], or [LSB, DATA, DATA, MSB]
// and returns a combined u32 value.
pub(crate) fn bytes_to_u32(b: &[u8]) -> u32 {
    if b.len() > 4 {
        log::error!("Slice larger than 4. Returning just first four bytes combined");
    }
    match b.len() {
        1 => b[0] as u32,
        2 => (b[0] as u32) + ((b[1] as u32) << 8),
        3 => (b[0] as u32) + ((b[1] as u32) << 8) + ((b[2] as u32) << 16),
        _ => (b[0] as u32) + ((b[1] as u32) << 8) + ((b[2] as u32) << 16) + ((b[3] as u32) << 24),
    }
}

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn test_new() {
        let data = vec![0; 5];
        let m = Message::new(0, &data);
        assert_eq!(m.id, 0);
        assert_eq!(m.data, vec![0; 5]);
    }

    #[test]
    fn test_read_buffer() {
        let startup_message = Message::new(MESG_STARTUP_MESG_ID, &[0x00]);
        let mut buffer = startup_message.encode();
        buffer.extend_from_slice(&startup_message.encode()[..]);
        buffer.extend_from_slice(&startup_message.encode()[..]);
        let mut read_buffer = ReadBuffer::new(&buffer[..]);
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(read_buffer.next(), None);
    }

    #[test]
    fn test_read_buffer_with_invalid_data() {
        let startup_message = Message::new(MESG_STARTUP_MESG_ID, &[0x00]);
        let mut buffer = startup_message.encode();
        buffer.extend_from_slice(&startup_message.encode()[..]);
        buffer.extend_from_slice(&[0, 1, 2, 3]);
        buffer.extend_from_slice(&startup_message.encode()[..]);
        let mut read_buffer = ReadBuffer::new(&buffer[..]);
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(read_buffer.next(), None);
    }

    #[test]
    fn test_read_buffer_with_invalid_mesg() {
        let startup_message = Message::new(MESG_STARTUP_MESG_ID, &[0x00]);
        let mut buffer = startup_message.encode();
        buffer.extend_from_slice(&startup_message.encode()[..]);
        buffer.extend_from_slice(&[MESG_TX_SYNC, 1, 2, 0]);
        buffer.extend_from_slice(&startup_message.encode()[..]);
        let mut read_buffer = ReadBuffer::new(&buffer[..]);
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(
            read_buffer.next(),
            Some(Response::Startup(StartupMessage(0x00)))
        );
        assert_eq!(read_buffer.next(), None);
    }

    #[test]
    fn test_startup_message() {
        assert_eq!(StartupMessage(0).reason(), StartupReason::PowerOnReset);
        assert_eq!(
            StartupMessage(0x01).reason(),
            StartupReason::HardwareResetLine
        );
        assert_eq!(StartupMessage(0x02).reason(), StartupReason::WatchDogReset);
        assert_eq!(StartupMessage(0x20).reason(), StartupReason::CommandReset);
        assert_eq!(
            StartupMessage(0x40).reason(),
            StartupReason::SynchronousReset
        );
        assert_eq!(StartupMessage(0x80).reason(), StartupReason::SuspendReset);
        assert_eq!(StartupMessage(0x95).reason(), StartupReason::Error);
    }

    #[test]
    fn test_encode() {
        let data = vec![1, 0xac, 2, 0x5c, 3];
        let len = data.len();
        let m = Message::new(MESG_CAPABILITIES_ID, &data);
        let buf = m.encode();
        let mut checksum = 0;
        let total_size = buf.len() - 3;
        for i in 0..total_size {
            checksum ^= buf[i];
        }
        assert_eq!(buf[0], MESG_TX_SYNC);
        assert_eq!(buf[1], len as u8);
        //MESG_CAPABILITIES_ID = 0x54
        assert_eq!(buf[2], 0x54);
        assert_eq!(buf[3..8], data[..]);
        assert_eq!(buf[total_size], checksum);
    }

    #[test]
    fn test_process_message() {
        let startup_message = Message::new(MESG_STARTUP_MESG_ID, &[0x00]);
        let buf = startup_message.encode();
        let mesg = process_message(&buf);
        assert_eq!(mesg, Response::Startup(StartupMessage(0x00)));
    }

    // The following tests test message creation. Since we use constants
    // for the ID, we want to assert against the value of the constant.
    // This way if the value of the constant is changed above, the test will
    // fail without a subsequent change of value here. Since these values
    // are part of the ANT+ spec, these values should not change unless there
    // is a breaking change in the ANT+ spec.
    //
    // If we just asserted against the constant value, a change above would
    // continue to pass here when it should fail.
    #[test]
    fn test_reset_message() {
        let mesg = reset();
        //MESG_RESET = 0x4A
        assert_eq!(mesg.id, 0x4A);
        assert_eq!(mesg.data[..], [0; 15]);
    }

    #[test]
    fn test_set_network_key_message() {
        let key = vec![0; 8];
        let mesg = set_network_key(0, &key);
        // MESG_NETWORK_KEY_ID = 0x46
        assert_eq!(mesg.id, 0x46);
        assert_eq!(mesg.data[..], [0; 9]);
    }

    #[test]
    fn test_get_capabilities_message() {
        let mesg = get_capabilities();
        // MESG_REQUEST = 0x4D
        // MESG_CAPABILITIES_ID = 0x54
        assert_eq!(mesg.id, 0x4D);
        assert_eq!(mesg.data[..], [0, 0x54]);
    }

    #[test]
    fn get_channel_id_message() {
        let mesg = get_channel_id(0);
        // MESG_REQUEST = 0x4D
        // MESG_CHANNEL_ID_ID = 0x51
        assert_eq!(mesg.id, 0x4D);
        assert_eq!(mesg.data[..], [0, 0x51]);
    }

    #[test]
    fn assign_channel_message() {
        let mesg = assign_channel(0, 0, 0);
        // MESG_ASSIGN_CHANNEL_ID = 0x42
        assert_eq!(mesg.id, 0x42);
        assert_eq!(mesg.data[..], [0, 0, 0]);
    }

    #[test]
    fn set_channel_id_message() {
        let mesg = set_channel_id(0, 1000, 0x78, 0);
        // MESG_CHANNEL_ID_ID = 0x51
        assert_eq!(mesg.id, 0x51);
        assert_eq!(mesg.data[0], 0);
        assert_eq!(mesg.data[1], (1000 & 0xFF) as u8);
        assert_eq!(mesg.data[2], ((1000 >> 8) & 0xFF) as u8);
        assert_eq!(mesg.data[3], 0x78);
        assert_eq!(mesg.data[4], 0);
    }

    #[test]
    fn set_hp_search_timeout_message() {
        let mesg = set_hp_search_timeout(0, 30);
        // MESG_CHANNEL_SEARCH_TIMEOUT_ID = 0x44
        assert_eq!(mesg.id, 0x44);
        assert_eq!(mesg.data[..], [0, 30]);
    }

    #[test]
    fn set_channel_period_message() {
        let mesg = set_channel_period(0, 8070);
        // MESG_CHANNEL_MESG_PERIOD_ID = 0x43
        assert_eq!(mesg.id, 0x43);
        assert_eq!(mesg.data[0], 0);
        assert_eq!(mesg.data[1], (8070 & 0xFF) as u8);
        assert_eq!(mesg.data[2], ((8070 >> 8) & 0xFF) as u8);
    }

    #[test]
    fn set_channel_frequency_message() {
        let mesg = set_channel_frequency(0, 0x39);
        // MESG_CHANNEL_RADIO_FREQ_ID = 0x45
        assert_eq!(mesg.id, 0x45);
        assert_eq!(mesg.data[..], [0, 0x39]);
    }

    #[test]
    fn open_channel_message() {
        let mesg = open_channel(0);
        // MESG_OPEN_CHANNEL_ID = 0x4B
        assert_eq!(mesg.id, 0x4B);
        assert_eq!(mesg.data[..], [0]);
    }

    #[test]
    fn close_channel_message() {
        let mesg = close_channel(0);
        // MESG_CLOSE_CHANNEL_ID = 0x4C
        assert_eq!(mesg.id, 0x4C);
        assert_eq!(mesg.data[..], [0]);
    }

    #[test]
    fn unassign_channel_message() {
        let mesg = unassign_channel(0);
        // MESG_UNASSIGN_CHANNEL_ID = 0x41
        assert_eq!(mesg.id, 0x41);
        assert_eq!(mesg.data[..], [0]);
    }
}
