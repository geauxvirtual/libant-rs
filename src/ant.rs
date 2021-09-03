//use crossbeam_channel::{unbounded, Receiver, Sender};
use crossbeam_channel::{Receiver, Sender};

use super::Result;
use crate::{
    channel::Channel,
    device::Device,
    error::AntError,
    message::{self, ChannelResponseCode, Message, Response},
    usb::{UsbContext, UsbDevice},
};

use log::{debug, error, trace};

// Doing this for now, but ANT_NETWORK_KEY will most likely be passed in
// when Ant::init is called. Whatever app is using this library may be
// responsible for passing in the ANT_NETWORK_KEY.
const ANT_NETWORK: u8 = 1;
const ANT_NETWORK_KEY: [u8; 8] = [0xB9, 0xA5, 0x21, 0xFB, 0xBD, 0x72, 0xC3, 0x45];

#[derive(Debug, PartialEq)]
enum State {
    NotReady,
    Reset,
    SetNetworkKey,
    Running,
}

//struct Channel {
//    rx: Receiver<Message>,
//    tx: Sender<Message>,
//}

//impl Channel {
//    fn new() -> Self {
//        let (tx, rx) = unbounded();
//        Channel { rx, tx }
//    }
//}

pub enum Request {
    OpenChannel(u8, Device),
    CloseChannel(u8),
    Send(Message),
    Quit,
}

pub struct Ant<T: UsbContext> {
    usb_device: UsbDevice<T>,
    state: State,
    request: Receiver<Request>,
    message: Sender<Response>,
    channels: Vec<Channel>,
    //    messages: Channel,
}

impl<T: UsbContext> Ant<T> {
    pub fn init(ctx: &mut T, rx: Receiver<Request>, tx: Sender<Response>) -> Result<Ant<T>> {
        Ok(Ant {
            usb_device: UsbDevice::init(ctx)?,
            state: State::NotReady,
            request: rx,
            channels: vec![],
            message: tx,
            //            messages: Channel::new(),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // Check to see if we're already running
        if self.state == State::Running {
            return Err(AntError::AlreadyRunning);
        }
        // ANT+ run loop to process/send messages
        let mut reset_attempts = 0;
        loop {
            // See if there are any messages to read
            match self.usb_device.read() {
                Ok(mut buffer) => {
                    for mesg in buffer.next() {
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
                            // TODO: Loop through existing channels to see if channel
                            // is already assigned. If it is, return an error
                            // back on message channel. If it doesn't exist,
                            // push channel into channels vec, and then send a
                            // message to assign the channel.
                            let channel = Channel::new(number, device);
                            // handle error
                            let _ = self.usb_device.write(&channel.assign(ANT_NETWORK).encode());
                            self.channels.push(channel);
                        }
                        Request::CloseChannel(number) => {
                            for c in self.channels.iter() {
                                if number == c.number() {
                                    debug!("Closing channel {}", number);
                                    let _ = self
                                        .usb_device
                                        .write(&message::close_channel(number).encode());
                                }
                            }
                        }
                        Request::Send(mesg) => {
                            let _ = self.usb_device.write(&mesg.encode());
                        }
                        Request::Quit => return Ok(()),
                    },
                    Err(crossbeam_channel::TryRecvError::Disconnected) => break,
                    Err(_) => continue,
                }
            }
        }
        Ok(())
    }

    fn route(&mut self, message: &Response) {
        match self.state {
            State::NotReady => {} // Drop message
            State::Reset => match message {
                Response::Startup(_mesg) => {
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
                Response::Startup(_mesg) => self.state = State::Reset,
                Response::ChannelResponse(mesg) => {
                    if mesg.code() == ChannelResponseCode::ResponseNoError {
                        debug! {"Setting state to Running"};
                        self.state = State::Running;
                    }
                }
                _ => {}
            },
            State::Running => match message {
                Response::Startup(_mesg) => self.state = State::Reset,
                Response::ChannelResponse(mesg) => {
                    // Check to see if we have an event
                    if mesg.message_id() == 1 {
                        trace!("Event received: {:x?}", mesg);
                        return;
                        //unimplemented!();
                    }
                    // TODO: There will be other codes, but for now just have one.
                    // Currently if something else is received, the code will
                    // panic until we add support for it. Happy path for now.
                    match mesg.code() {
                        ChannelResponseCode::ResponseNoError => {
                            for c in &mut self.channels {
                                if c.number() == mesg.channel() {
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
                        }
                        ChannelResponseCode::EventChannelClosed => {
                            debug!("Channel closed");
                        }
                        ChannelResponseCode::ChannelInWrongState => {
                            debug!("Channel in wrong state");
                        }
                    }
                }
                Response::BroadcastData(mesg) => self
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

    //    pub fn send(&self, m: Message) -> Result<()> {
    //        self.requests
    //            .tx
    //            .send(m)
    //            .map_err(|e| AntError::RequestSendError(e))
    //    }
    //
    //    pub fn try_recv(&self) -> Result<Message> {
    //        self.messages
    //            .rx
    //            .try_recv()
    //            .map_err(|e| AntError::MessageTryRecvError(e))
    //    }
}
