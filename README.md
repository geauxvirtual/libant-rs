## About

libant-rs is a Rust library for interacting with ANT+ devices. The goal of this library is to provide a simple
interface for configuring an ANT+ USB device to allow for device specific channel configuration in order to
receive broadcast data from an ANT+ device.

## Example

```rust,no_run
// libant provides the ability to create unbounded Crossbeam channels for passing requests
// to the run loop while receiving data back from the run loop.

let (request_tx, request_rx) = libant::unbounded();
let (message_tx, message_rx) = libant::unbounded();

// This starts our run loop in a separate thread. The run loop can be stopped by sending a Request::Quit message
let run_handle = std::thread::spawn(move || libant::ant::run(request_rx, message_tx));

// Configure a channel for a specific type of device.

use libant::device::{Device, hrm::HeartRateMonitor};
use libant::{Request, Response};

let mut hrm = HeartRateMonitor::new();
let device = Device::HeartRateMonitor(hrm.clone());
request_tx.send(Request::OpenChannel(0, device)).unwrap();

// Now that the channel is open, process any broadcast data messages
loop {
    match message_rx.recv() {
        Ok(Response::BroadcastData(mesg)) => {
            hrm.decode_broadcast_data(mesg.data));
            println!("Heartrate: {}", hrm.heartrate();
        }
    }
}
```

## Testing

Library has been tested on Mac OS X, but *should* work on any platform that libusb compiles on.

## TODOs

- [ ] Add support for powermeters
- [ ] Add support for electronic trainers
- [ ] Add more error handling

