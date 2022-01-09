// TODO Go through this and figure out legacy code from new code.
/// Message module provides a way for creating messages to send to the ANT+
/// USB device or ANT+ device along with providing a way to decode messages
/// received from the ANT+ USB device or ANT+ device sending data on a channel.
use log::debug;
use std::convert::TryInto;

// Starting to figure out legacy vs what's needed to support what this library
// is being used for right now. I know some of these values and names were taken
// from the ANT+ SDK.
const ANT_STANDARD_DATA_PAYLOAD_SIZE: usize = 8;
const ANT_EXT_MESG_DEVICE_ID_FIELD_SIZE: usize = 4;
const ANT_EXT_STRING_SIZE: usize = 27;

const MESG_TX_SYNC: u8 = 0xA4;
const MESG_RX_SYNC: u8 = 0xA5;
const MESG_SYNC_SIZE: usize = 1;
const MESG_SIZE_SIZE: usize = 1;
const MESG_ID_SIZE: usize = 1;
const MESG_CHANNEL_NUM_SIZE: usize = 1;
const MESG_EXT_MESG_BF_SIZE: usize = 1;
const MESG_CHECKSUM_SIZE: usize = 1;
const MESG_DATA_SIZE: usize = 9;

const MESG_ANT_MAX_PAYLOAD_SIZE: usize = ANT_STANDARD_DATA_PAYLOAD_SIZE;
const MESG_MAX_EXT_DATA_SIZE: usize = ANT_EXT_MESG_DEVICE_ID_FIELD_SIZE + ANT_EXT_STRING_SIZE;
const MESG_MAX_DATA_SIZE: usize =
    MESG_ANT_MAX_PAYLOAD_SIZE + MESG_EXT_MESG_BF_SIZE + MESG_MAX_EXT_DATA_SIZE;
const MESG_MAX_SIZE_VALUE: usize = MESG_MAX_DATA_SIZE + MESG_CHANNEL_NUM_SIZE;
const MESG_BUFFER_SIZE: usize =
    MESG_SIZE_SIZE + MESG_ID_SIZE + MESG_CHANNEL_NUM_SIZE + MESG_MAX_DATA_SIZE + MESG_CHECKSUM_SIZE;
const MESG_FRAMED_SIZE: usize = MESG_ID_SIZE + MESG_CHANNEL_NUM_SIZE + MESG_MAX_DATA_SIZE;
const MESG_HEADER_SIZE: usize = MESG_SYNC_SIZE + MESG_SIZE_SIZE + MESG_ID_SIZE;
const MESG_FRAME_SIZE: usize = MESG_HEADER_SIZE + MESG_CHECKSUM_SIZE;
const MESG_MAX_SIZE: usize = MESG_MAX_DATA_SIZE + MESG_FRAME_SIZE;
const MESG_SIZE_OFFSET: usize = MESG_SYNC_SIZE;
const MESG_ID_OFFSET: usize = MESG_SYNC_SIZE + MESG_SIZE_SIZE;
const MESG_DATA_OFFSET: usize = MESG_HEADER_SIZE;
const MESG_RECOMMENDED_BUFFER_SIZE: u8 = 64;

const RESPONSE_NO_ERROR: u8 = 0x00;
const MESG_EVENT_ID: u8 = 0x01;
const MESG_RESPONSE_EVENT_ID: u8 = 0x40;
const MESG_UNASSIGN_CHANNEL_ID: u8 = 0x41;
pub const MESG_ASSIGN_CHANNEL_ID: u8 = 0x42;
pub const MESG_CHANNEL_MESG_PERIOD_ID: u8 = 0x43;
pub const MESG_CHANNEL_SEARCH_TIMEOUT_ID: u8 = 0x44;
pub const MESG_CHANNEL_RADIO_FREQ_ID: u8 = 0x45;
const MESG_NETWORK_KEY_ID: u8 = 0x46;
const MESG_RESET: u8 = 0x4A;
pub const MESG_OPEN_CHANNEL_ID: u8 = 0x4B;
const MESG_CLOSE_CHANNEL_ID: u8 = 0x4C;
const MESG_REQUEST: u8 = 0x4D;
const MESG_BROADCAST_DATA_ID: u8 = 0x4E;
const MESG_ACKNOWLEDGE_DATA_ID: u8 = 0x4F;
pub const MESG_CHANNEL_ID_ID: u8 = 0x51;
const MESG_CAPABILITIES_ID: u8 = 0x54;
const MESG_STARTUP_MESG_ID: u8 = 0x6F;
const MESG_CREATE_CHANNEL_ID: u8 = 0xFE;
// Not part of ANT+ standard. Using as control message for quitting
const MESG_QUIT: u8 = 0xFF;

const EVENT_RX_SEARCH_TIMEOUT: u8 = 0x01;
const EVENT_CHANNEL_CLOSED: u8 = 0x07;
const CHANNEL_IN_WRONG_STATE: u8 = 0x15;

/// ReadBuffer provides a buffer to through data received from the ANT+ USB device and turn
/// the data into a Message
pub struct ReadBuffer {
    index: usize,
    inner: [u8; 512],
    len: usize,
}

impl ReadBuffer {
    pub fn new() -> Self {
        ReadBuffer {
            index: 0,
            inner: [0; 512],
            len: 0,
        }
    }

    pub fn len(&mut self, len: usize) {
        self.len = len;
    }

    pub fn inner_as_mut(&mut self) -> &mut [u8] {
        &mut self.inner
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
            if self.index >= self.len {
                self.index = 0;
                self.len = 0;
                return None;
            }
            if self.inner[self.index] == MESG_TX_SYNC {
                let index = self.index;
                // Length of message
                let len = index + self.inner[index + 1] as usize + 4;
                // Verify checksum
                if checksum(&self.inner[index..len]) == 0 {
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

#[derive(Clone, Debug, PartialEq)]
pub struct BroadcastDataMessage {
    channel_id: u8,
    data: [u8; 8],
}

impl BroadcastDataMessage {
    // Maybe change this to try_from and return an error
    pub fn from(mesg: &[u8]) -> Self {
        Self {
            channel_id: mesg[0],
            data: mesg[1..].try_into().unwrap(),
        }
    }

    pub fn channel(&self) -> u8 {
        self.channel_id
    }

    pub fn data(self) -> [u8; 8] {
        self.data
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

// Message is the low-level representation of a message to send to the ANT+ USB
// stick or ANT+ device.
// id: Type of message being transmitted.
// data: Data payload to transmit with the message. Data length varies based on type
// of message being transmitted.
//
// Byte layout of a message where N is length of data payload
// [0] - Sync byte
// [1] - Size of data payload
// [2] - ID of the message
// [3..N+2] - Data payload
// [N+3] - Checksum

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
        let size = MESG_HEADER_SIZE + self.data.len() + MESG_CHECKSUM_SIZE;
        let mut buf: Vec<u8> = vec![0; size];
        buf[0] = MESG_TX_SYNC;
        buf[MESG_SIZE_OFFSET] = self.data.len() as u8;
        buf[MESG_ID_OFFSET] = self.id;
        buf[MESG_DATA_OFFSET..(size - 1)].copy_from_slice(&self.data);

        // Calculate checksum and store it in the last byte of the message.
        // Checksum is the XOR of all bytes of the message.
        buf[size - 1] = checksum(&buf[..size - 1]);
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

fn checksum(buf: &[u8]) -> u8 {
    buf[1..].iter().fold(buf[0], |acc, x| acc ^ x)
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

// App message to quit our threads for now
pub fn quit() -> Message {
    Message::new(MESG_QUIT, &[0])
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
        let mut read_buffer = ReadBuffer::new();
        let startup_message = Message::new(MESG_STARTUP_MESG_ID, &[0x00]);
        let mut buffer = startup_message.encode();
        buffer.extend_from_slice(&startup_message.encode()[..]);
        buffer.extend_from_slice(&startup_message.encode()[..]);
        read_buffer.inner_as_mut()[..buffer.len()].copy_from_slice(&buffer[..]);
        read_buffer.len(buffer.len());
        //let mut read_buffer = ReadBuffer::new(&buffer[..]);
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
        let mut read_buffer = ReadBuffer::new();
        let startup_message = Message::new(MESG_STARTUP_MESG_ID, &[0x00]);
        let mut buffer = startup_message.encode();
        buffer.extend_from_slice(&startup_message.encode()[..]);
        buffer.extend_from_slice(&[0, 1, 2, 3]);
        buffer.extend_from_slice(&startup_message.encode()[..]);
        read_buffer.inner_as_mut()[..buffer.len()].copy_from_slice(&buffer[..]);
        read_buffer.len(buffer.len());
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
        let mut read_buffer = ReadBuffer::new();
        let startup_message = Message::new(MESG_STARTUP_MESG_ID, &[0x00]);
        let mut buffer = startup_message.encode();
        buffer.extend_from_slice(&startup_message.encode()[..]);
        buffer.extend_from_slice(&[MESG_TX_SYNC, 1, 2, 0]);
        buffer.extend_from_slice(&startup_message.encode()[..]);
        read_buffer.inner_as_mut()[..buffer.len()].copy_from_slice(&buffer[..]);
        read_buffer.len(buffer.len());
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
        let data = [MESG_TX_SYNC, 5, MESG_CAPABILITIES_ID, 1, 0xac, 2, 0x5c, 3];
        let m = Message::new(data[2], &data[3..]);
        let buf = m.encode();
        let checksum = checksum(&data);
        assert_eq!(buf[0], data[0]);
        assert_eq!(buf[1], data[1]);
        //MESG_CAPABILITIES_ID = 0x54
        assert_eq!(buf[2], data[2]);
        assert_eq!(buf[3..8], data[3..]);
        assert_eq!(buf[8], checksum);
    }

    #[test]
    fn test_checksum() {
        assert_eq!(checksum(&[2, 3]), 1);
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
