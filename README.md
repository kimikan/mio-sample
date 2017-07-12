# miox+
it's the multi-thread version of miox.

A mio based tcpserver 
Client Test code:




<src>
extern crate byteorder;

use std::io::prelude::*;
use std::net::TcpStream;
use std::thread;

use byteorder::{ByteOrder, BigEndian};


fn main() {

    for i in 0..10 {

        let _ = thread::spawn(move|| {

            let mut stream = TcpStream::connect("127.0.0.1:7777").unwrap();

            loop {
                let msg = format!("test message: {}", i);
                let mut buf = [0u8; 8];
                BigEndian::write_u64(&mut buf, msg.len() as u64);
                stream.write_all(buf.as_ref()).unwrap();
                stream.write_all(msg.as_ref()).unwrap();

                let mut buf = [0u8; 8];
                stream.read(&mut buf).unwrap();

                let msg_len = BigEndian::read_u64(&mut buf);

                let mut r = [0u8; 256];
                let s_ref = <TcpStream as Read>::by_ref(&mut stream);

                match s_ref.take(msg_len).read(&mut r) {
                    Ok(0) => {
                        println!("thread {}: 0 bytes read", i);
                    },
                    Ok(n) => {
                        println!("thread {}: {} bytes read", i, n);

                        let s = std::str::from_utf8(&r[..]).unwrap();
                        println!("thread {} read = {}", i, s);
                    },
                    Err(e) => {
                        panic!("thread {}: {}", i, e);
                    }
                }
            }
        });
    }

    loop {}
}

</src>
