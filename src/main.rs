
extern crate mio;
extern crate byteorder;
extern crate slab;

mod server;
mod connection;
mod poll;

use std::thread;

fn main() {
    let s = server::ServerContext::new("0.0.0.0:7777", 127);

    if let Some(ctx) = s {
        let server  =  server::Server::new();
        
        let mut handles = vec![];
        for _ in 0..3 {
            let clone = ctx.clone();
            let mut server_clone = server.clone();
            handles.push(thread::spawn(move ||{
                server_clone.run(&clone).expect("server run failed");
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
            
    }
    println!("app exit!");
}
