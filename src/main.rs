
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

fn main() {
    let s = context::ServerContext::new("127.0.0.1:7777", 127);

    if let Some(ctx) = s {
        let server = server::Server::new(EchoHandler::new());

        let mut handles = vec![];
        for _ in 0..3 {
            let clone = ctx.clone();
            let mut server_clone = server.clone();
            handles.push(thread::spawn(move || {
                                           server_clone.run(&clone).expect("server run failed");
                                       }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

    }
    println!("app exit!");
}
