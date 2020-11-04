//use crossbeam_channel::{unbounded, Receiver, Sender};

use super::Result;
use crate::{
    error::AntError,
    message::{self, ChannelResponseCode, Response},
    usb::{UsbContext, UsbDevice},
};

// Doing this for now, but ANT_NETWORK_KEY will most likely be passed in
// when Ant::init is called. Whatever app is using this library may be
// responsible for passing in the ANT_NETWORK_KEY.
pub const ANT_NETWORK: u8 = 1;
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

pub struct Ant<T: UsbContext> {
    usb_device: UsbDevice<T>,
    state: State,
    //    requests: Channel,
    //    messages: Channel,
}

impl<T: UsbContext> Ant<T> {
    pub fn init(ctx: &mut T) -> Result<Ant<T>> {
        Ok(Ant {
            usb_device: UsbDevice::init(ctx)?,
            state: State::NotReady,
            //            requests: Channel::new(),
            //            messages: Channel::new(),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // Check to see if we're already running
        if self.state == State::Running {
            return Err(AntError::AlreadyRunning);
        }
        // ANT+ run loop to process/send messages
        loop {
            // See if there are any messages to read
            match self.usb_device.read() {
                Ok(mut buffer) => {
                    for mesg in buffer.next() {
                        self.route(&mesg)
                    }
                }
                Err(AntError::UsbDeviceError(rusb::Error::Timeout)) => match self.state {
                    State::NotReady => self.state = State::Reset,
                    State::Reset => self.reset()?,
                    _ => {}
                },
                Err(e) => return Err(e),
            }
            // Messages handled, let's see if there are any requests to
            // operate on.
        }
    }

    fn route(&mut self, message: &Response) {
        match self.state {
            State::NotReady => {} // Drop message
            State::Reset => {
                match message {
                    Response::Startup(_mesg) => {
                        self.state = State::SetNetworkKey;
                        if let Err(_e) = self.set_network_key() {
                            self.state = State::Reset;
                        }
                    }
                    _ => {} // Drop message
                }
            }
            State::SetNetworkKey => match message {
                Response::Startup(_mesg) => self.state = State::Reset,
                Response::ChannelResponse(mesg) => {
                    if mesg.code() == ChannelResponseCode::ResponseNoError {
                        self.state = State::Running;
                    }
                }
            },
            State::Running => match message {
                Response::Startup(_mesg) => self.state = State::Reset,
                Response::ChannelResponse(_mesg) => unimplemented!(),
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
