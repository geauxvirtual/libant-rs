/// The Ant module provides the main run() function that when called will startup
/// the ANT+ USB device if found and configure it to be ready to accept channel configurations.
use crossbeam_channel::{Receiver, Sender};

use super::Result;
use crate::{
    channel::{Channel, Config},
    error::AntError,
    message::Response as DeviceResponse,
    message::{self, BroadcastDataMessage, ChannelResponseCode, Message, ReadBuffer},
    usb::{UsbContext, UsbDevice},
};

use log::{debug, error, info, trace};

// Default to ANT network 1. The ANT+ USB device can support up to three networks, and appears
// through testing that devices work on ANT network 1 even though 0 is the public network.
const ANT_NETWORK: u8 = 1;
const ANT_NETWORK_KEY: [u8; 8] = [0xB9, 0xA5, 0x21, 0xFB, 0xBD, 0x72, 0xC3, 0x45];

// Manages the state of the ANT+ USB devices.
#[derive(Debug, PartialEq)]
enum State {
    NotReady,
    Reset,
    SetNetworkKey,
    Running,
}

// TODO: Rename this to Command
/// Requests that can be sent to the run loop to either Open/Close a channel, send a message to an
/// ANT+ device, or Quit the loop closing all open channels.
pub enum Request {
    OpenChannel(u8, Config),
    CloseChannel(u8),
    Send(Message),
    Quit,
}

/// Responses that can be sent out of the run loop. BroadcastData from an ANT+ device or any types
/// of error that should be handled by the upstream application.
#[derive(Debug)]
pub enum Response {
    BroadcastData(BroadcastDataMessage),
    Error(AntError),
}

/// run is a public function that handles getting a USB context and
/// initializing the ANT+ stick. Errors are returned through the transmit side
/// of the ant message channel passed in. In the case of the USB context, if
/// an error is received, an error will be sent back on the channel and then
/// the function will return. If an error is received trying to initialize
/// the ANT+ stick, the error will be returned on the transmit channel and the
/// fucnction will continue to loop as some errors could involve the ANT+ stick
/// not being plugged in. When the ANT+ stick can be initialized, the function
/// will call ANT::init().run() that will reset and startup the ANT+ stick
/// and get it ready for communication.
pub fn run(rx: Receiver<Request>, tx: Sender<Response>) {
    // Get the USB context. If there is an error, send an Error
    // response over the transmit channel and return.
    let mut ctx = match crate::Context::new() {
        Ok(ctx) => ctx,
        Err(e) => {
            error!("Error getting USB Context: {:?}", e);
            // unwrap() is called here as the only error we should receive
            // here is if the other end of the channel has disconnected, i.e.
            // the other thread either no longer exists, or the channel has been
            // dropped. Without a transmit channel, nothing can actually be done
            // so might as well panic to kill this thread.
            tx.send(Response::Error(AntError::UsbDeviceError(e)))
                .unwrap();
            return;
        }
    };

    // Loop here looking for the ANT+ stick. If the user has not plugged
    // in the ANT+ stick, check every 1 second.
    let usb_device = loop {
        match UsbDevice::init(&mut ctx) {
            Ok(device) => break device,
            Err(e) => {
                error!("Error initializing ANT+ USB stick: {:?}", e);
                tx.send(Response::Error(e)).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }
    };

    // Initialize and run our ANT+ message loop, returning any errors
    // received back through transmit channel.
    if let Err(e) = Ant::init(usb_device, rx, tx.clone()).run() {
        tx.send(Response::Error(e)).unwrap();
    }
}

pub struct Ant<T: UsbContext> {
    usb_device: UsbDevice<T>,
    state: State,
    request: Receiver<Request>,
    message: Sender<Response>,
    // By default we support 8 channels. A typical device could support 3 networks of 8 channels
    // each, but from testing ANT+ devices I have, they only send data on one network, so only
    // configure for 8 channels.
    channels: [Option<Channel>; 8],
}

impl<T: UsbContext> Ant<T> {
    pub fn init(usb_device: UsbDevice<T>, rx: Receiver<Request>, tx: Sender<Response>) -> Ant<T> {
        Ant {
            usb_device,
            state: State::NotReady,
            request: rx,
            message: tx,
            channels: Default::default(),
        }
    }

    // This is the main run called after initializing the ANT+ USB device. It handles reading data
    // from the ANT+ USB device and handling the message, whether its part of the initial
    // configuration, channel configuration, or broadcast data. If there are no messages to read,
    // the state of the system will decide if the system needs to be configured, or if configured,
    // check to see if any messages are waiting to be sent. Should a channel close due to error or
    // timeout, and the channel is still known, then the channel will be reopened until a request
    // is sent to close the channel by an upstream application.
    pub fn run(&mut self) -> Result<()> {
        // Check to see if we're already running
        if self.state == State::Running {
            return Err(AntError::AlreadyRunning);
        }
        // ANT+ run loop to process/send messages
        let mut read_buffer = ReadBuffer::new();
        let mut reset_attempts = 0;
        loop {
            // See if there are any messages to read
            match self.usb_device.read(read_buffer.inner_as_mut()) {
                Ok(len) => {
                    read_buffer.len(len);
                    for mesg in &mut read_buffer {
                        trace! {"Routing message response: {:x?}", mesg};
                        self.route(&mesg)
                    }
                }
                Err(AntError::UsbDeviceError(rusb::Error::Timeout)) => match self.state {
                    State::NotReady => {
                        debug! {"Setting state to Reset"};
                        self.state = State::Reset;
                    }
                    State::Reset => {
                        // From time to time, ANT+ sticks may not respond
                        // to reset requests, especially if open channels
                        // were not closed or unassigned prior to the
                        // thread exiting. If the ANT+ stick gets stuck
                        // in this state,no messages will be received and acted
                        // on, and reset messages will just continue to be sent.
                        // This is configured to try three times then exitt
                        if reset_attempts < 2 {
                            debug! {"Sending reset command"};
                            self.reset()?;
                            reset_attempts += 1;
                        } else {
                            return Err(AntError::Reset);
                        }
                    }
                    _ => {}
                },
                Err(e) => return Err(e),
            }
            // Messages handled, let's see if there are any requests to
            // operate on. We only handle requests once in the running state
            if let State::Running = self.state {
                match self.request.try_recv() {
                    Ok(request) => match request {
                        Request::OpenChannel(number, device) => {
                            if self.channels[number as usize].is_some() {
                                error!("Channel {} already exists", number);
                                self.message
                                    .send(Response::Error(AntError::ChannelExists(number)))
                                    .unwrap();
                                continue;
                            }
                            // TODO: Handle error properly. For now, we'll just unwrap
                            // so the thread panics if there are any issues
                            // writing out to the ANT+ stick
                            let channel = Channel::new(number, device);
                            self.usb_device
                                .write(&channel.assign(ANT_NETWORK).encode())
                                .unwrap();
                            self.channels[number as usize] = Some(channel);
                        }
                        Request::CloseChannel(number) => {
                            if self.channels[number as usize].is_some() {
                                debug!("Closing channel {}", number);
                                self.usb_device
                                    .write(&message::close_channel(number).encode())
                                    .unwrap();
                                self.channels[number as usize] = None
                            }
                        }
                        Request::Send(mesg) => {
                            //let _ = self.usb_device.write(&mesg.encode());
                            self.usb_device.write(&mesg.encode()).unwrap();
                        }
                        Request::Quit => {
                            self.reset()?;
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            return Ok(());
                        }
                    },
                    Err(crossbeam_channel::TryRecvError::Disconnected) => break,
                    Err(_) => continue,
                }
            }
        }
        Ok(())
    }

    // Route handles what to do with the message based on the state of the system.
    fn route(&mut self, message: &DeviceResponse) {
        match self.state {
            State::NotReady => {} // Drop message
            State::Reset => match message {
                DeviceResponse::Startup(_mesg) => {
                    debug! {"Setting state to SetNetworkKey"};
                    self.state = State::SetNetworkKey;
                    debug! {"Setting network key"};
                    if let Err(e) = self.set_network_key() {
                        error! {"Error setting network key: {:?}", e};
                        debug! {"Setting state to Reset"};
                        self.state = State::Reset;
                    }
                }
                _ => debug!("{:x?}", message), // Drop message
            },
            State::SetNetworkKey => match message {
                DeviceResponse::Startup(_mesg) => self.state = State::Reset,
                DeviceResponse::ChannelResponse(mesg) => {
                    if mesg.code() == ChannelResponseCode::ResponseNoError {
                        debug! {"Setting state to Running"};
                        self.state = State::Running;
                    }
                }
                _ => {}
            },
            State::Running => match message {
                DeviceResponse::Startup(_mesg) => self.state = State::Reset,
                DeviceResponse::ChannelResponse(mesg) => {
                    // Check to see if we have an event
                    if mesg.message_id() == 1 {
                        match mesg.code() {
                            ChannelResponseCode::EventRxFail => {
                                trace!("EVENT_RX_FAIL received on channel {}", mesg.channel());
                            }
                            ChannelResponseCode::EventRxSearchTimeout => {
                                trace!(
                                    "EVENT_RX_SEARCH_TIMEOUT received on channel {}",
                                    mesg.channel()
                                );
                            }
                            ChannelResponseCode::EventRxFailGoToSearch => {
                                trace!(
                                    "EVENT_RX_FAIL_GO_TO_SEARCH received on channel {}",
                                    mesg.channel()
                                );
                            }
                            ChannelResponseCode::EventChannelClosed => {
                                // If a channel closed message is received, but the
                                // the channel was not requested to be closed, re-open
                                // the channel.
                                trace!(
                                    "EVENT_CHANNEL_CLOSED received on channel {}",
                                    mesg.channel()
                                );
                                let send_mesg = match &mut self.channels[mesg.channel() as usize] {
                                    Some(c) => {
                                        // If a channel closed message is received, but the
                                        // the channel was not requested to be closed, re-open
                                        // the channel.
                                        info!("Re-opening channel {}", mesg.channel());
                                        c.open().encode()
                                    }
                                    None => {
                                        // Unassign channel that was closed
                                        debug!("Unassigning channel {}", mesg.channel());
                                        crate::message::unassign_channel(mesg.channel()).encode()
                                    }
                                };
                                self.usb_device.write(&send_mesg).unwrap();
                            }
                            _ => {
                                trace!("Unhandled event received: {:x?}", mesg);
                            }
                        }
                        return;
                        //unimplemented!();
                    }
                    // TODO: There will be other codes, but for now just have one.
                    // Currently if something else is received, the code will
                    // panic until we add support for it. Happy path for now.
                    match mesg.code() {
                        ChannelResponseCode::ResponseNoError => {
                            if let Some(c) = &mut self.channels[mesg.channel() as usize] {
                                // Should use this to update state and then
                                // then configure the next message. We
                                // don't have a copy of the TX side of our
                                // request channel here. May have to rethink
                                // how that gets created and handled, or figure out
                                // a better way to send the next message.
                                if let Some(mesg) = c.route(&mesg) {
                                    let _ = self.usb_device.write(&mesg.encode());
                                }
                            }
                        }
                        ChannelResponseCode::ChannelInWrongState => {
                            trace!(
                                "CHANNEL_IN_WRONG_STATE received on channel {}",
                                mesg.channel()
                            );
                        }
                        _ => {
                            trace!("Unhandled channel response received: {:x?}", mesg);
                            return;
                        }
                    }
                }
                DeviceResponse::BroadcastData(mesg) => self
                    .message
                    .send(Response::BroadcastData(mesg.clone()))
                    .unwrap(),
            },
        }
    }

    fn reset(&self) -> Result<()> {
        self.usb_device.write(&message::reset().encode())?;
        std::thread::sleep(std::time::Duration::from_millis(500));
        Ok(())
    }

    fn set_network_key(&self) -> Result<()> {
        self.usb_device
            .write(&message::set_network_key(ANT_NETWORK, &ANT_NETWORK_KEY).encode())?;
        Ok(())
    }

    fn get_capabilities(&self) -> Result<()> {
        self.usb_device
            .write(&message::get_capabilities().encode())?;
        Ok(())
    }
}
