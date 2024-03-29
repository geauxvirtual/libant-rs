#![allow(dead_code)]
/// libant is a Rust implementation for interacting with ANT+ based devices.
/// The goal of this library is provide a simple interface for initializing
/// an ANT+ USB device by handling all the setup requirements to prepare the
/// ANT+ USB device to support configuring channels for different device types.
///
/// The libant run loop takes a request channel receive side for broadcast messages
/// to be sent to an ANT+ device or a Quit message to shut down the loop and a data channel
/// transmit side to send data out of the run loop, whether it's broadcast data or
/// event data.
///
/// let (request_tx, request_rx) = libant::unbounded();
/// let (message_tx, message_rx) = libant::unbounded();
///
/// In the application, the ant run loop can be started by the following since it is
/// a blocking process.
///
/// let run_handle = std::thread::spawn(move || libant::ant::run(request_rx, message_tx));
///
/// Internally, the library manages configured channels for channel configuration data only.
/// All broadcast data is sent directly to the client to handle decoding. The library does provide
/// helper methods for decoding broadcast data by the client application.
/// use libant::device::hrm::HeartRateMonitor;
/// use libant::{Request, Response};
///
/// let mut hrm = HeartRateMonitor::new();
/// request_tx.send(Request::OpenChannel(0, HeartRateMonitor::channel_config())).unwrap();
///
/// Broadcast and event messages can be read through the message_rx receive channel side.
///
/// loop {
///     match message_rx.recv() {
///         Ok(Response::BroadcastData(mesg)) => {
///             hrm.decode_broadcast_data(mesg.data());
///             // Do something with the device data that has now been decoded
///             println!("Heartrate: {}", hrm.heartrate());
///         }
///     }
/// }
///
/// To handle multiple devices, an enum can be utilized.
/// use libant::device::hrm::HeartRateMonitor;
/// use libant::device::powermeter::PowerMeter;
///
/// enum Device {
///     Hrm(HeartRateMonitor),
///     Pm(PowerMeter),
/// }
///
/// // ANT+ USB sticks support up to 8 channels.
/// let mut channels: [Option<Device>; 8] = Default::default();
///
/// channel[0] = Some(Device::Pm(PowerMeter::new());
/// channel[1] = Some(Device::Hrm(HeartRateMonitor::new());
///
/// request_tx.send(Request::OpenChannel(0, PowerMeter::channel_config())).unwrap();
/// request_tx.send(Request::OpenChannel(1, HeartRateMonitor::channel_config())).unwrap();
///
/// loop {
///     match message_rx.recv() {
///         Ok(Response::BroadcastData(mesg)) => {
///             let channel = mesg.channel() as usize;
///             match channels[channel] {
///                 Some(Device::Pm(ref mut device)) => { //decode data and handle it },
///                 Some(Device::Hrm(ref mut device)) => { // decode data and handle it },
///                 None => unimplemented!(),
///             }
///         }
///     }
/// }
pub mod ant;
pub mod channel;
pub mod device;
mod error;
pub mod message;
mod usb;

pub type Result<T> = std::result::Result<T, error::AntError>;

pub use ant::{Request, Response};
pub use crossbeam_channel::{unbounded, Receiver, Sender};
pub use usb::Context;
