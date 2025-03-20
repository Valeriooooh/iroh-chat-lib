//! This module is written for Rinf demonstrations.

use crate::messages::*;

pub async fn communicate() {
    // Send signals to Dart like below.
    let receiver = SomeNumber::get_dart_signal_receiver();
    while let Some(dart_signal) = receiver.recv().await {
        let message: SomeNumber = dart_signal.message;
        rinf::debug_print!("{message:?}");
        let send  = SmallNumber{number: message.number+1};
        send.send_signal_to_dart();
    }
}

// Though async tasks work, using the actor model
// is highly recommended for state management
// to achieve modularity and scalability in your app.
// To understand how to use the actor model,
// refer to the Rinf documentation.
