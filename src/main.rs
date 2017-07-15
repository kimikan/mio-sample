
/*written by kimikan, 2017-7-12*/
extern crate mio;
extern crate byteorder;
extern crate slab;

mod server;
mod connection;
mod poll;
mod context;
mod serialize;

use std::thread;
use std::sync::Arc;
use std::io;
use serialize::MessageHandler;
use connection::Connection;

struct EchoHandler {
    //nop
}

impl EchoHandler {
    fn new() -> EchoHandler {
        EchoHandler {}
    }
}

impl MessageHandler for EchoHandler {
    fn on_message_received(&self, c: &Connection, message: &Arc<Vec<u8>>) -> io::Result<()> {
        c.send_message(message.clone());
        Ok(())
    }
}

/* main usage */
fn main() {
    let listener = context::bind("127.0.0.1:7777").unwrap();
    let context = context::Context::new(EchoHandler::new(), /* max clients */127);

    let mut handles = vec![];
    for _ in 0..3 {
        let mut server = server::Server::new(listener.try_clone().unwrap()).unwrap();
        let ctx = context.clone();
        handles.push(thread::spawn(move || {
            server.run::<EchoHandler>(&ctx).expect("server run failed");
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    println!("app exit!");
}
