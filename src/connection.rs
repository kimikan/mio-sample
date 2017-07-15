
/*written by kimikan, 2017-7-12*/
use mio::Token;
use mio::net::TcpStream;

use byteorder::{ByteOrder, BigEndian};
use poll;

use std::sync::{Arc, RwLock};
use std::io;
use std::io::{Write, Read, Error, ErrorKind};

/* a client with an cnn*/
pub struct Connection {
    _token: Token,
    pub _stream: TcpStream,

    //cache the send message between events
    _send_queue: RwLock<Vec<Arc<Vec<u8>>>>,
    _read_next: usize,
    _write_next: bool,
}

impl Connection {
    /*
     * a connection means a tcpstream,  
     * and the token needed by the mio must be unique 
     * it 's managed by the server context.
     */
    pub fn new(stream: TcpStream, token: Token) -> Connection {
        Connection {
            _token: token,
            _stream: stream,
            _send_queue: RwLock::new(vec![]),
            _read_next: 0,
            _write_next: false,
        }
    }

    pub fn get_token(&self) -> Token {
        self._token
    }

    //result means, if read success, if fail, should cloase this
    //option means, got data?
    pub fn on_read(&mut self) -> io::Result<Option<Vec<u8>>> {
        let len_result = self.read_message_len();
        if let Ok(len) = len_result {
            if len <= 0 {
                //ewouldblock was returned.
                return Ok(None);
            } else {
                let mut vec: Vec<u8> = Vec::with_capacity(len);
                unsafe {
                    vec.set_len(len);
                }
                let read_result = self._stream.read(&mut vec);
                match read_result {
                    Err(e) => {
                        if e.kind() == ErrorKind::WouldBlock {
                            //cache the len. to continue read in next round
                            self._read_next = len;
                            return Ok(None);
                        } else {
                            println!("read error happend {:?}", e);
                            return Err(e);
                        }
                    }
                    Ok(n) => {
                        if n < len {
                            println!("error message number");
                            return Err(Error::new(ErrorKind::InvalidData, "error message number"));
                        }
                        //reset the next buffer
                        self._read_next = 0;
                        return Ok(Some(vec));
                    }
                } //end match?
            }
        } else {
            return Err(Error::new(ErrorKind::WriteZero, "msg len write failed"));
        }
    }

    /*
    * message =|message len| message buffer| 
    */
    fn read_message_len(&mut self) -> io::Result<usize> {
        if self._read_next > 0 {
            //_read_next,  means that not all of the data has been recevied
            //in previous around, so just make it whole completed
            return Ok(self._read_next);
        }
        let mut buf = [0u8; 8];
        let bytes = match self._stream.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    //0, means current read was blocked.
                    //it's not a error case,
                    //should not close socket.
                    return Ok(0);
                } else {
                    return Err(e);
                }
            }
        };

        if bytes < 8 {
            //the message len is u64, so it's 8 bytes long
            return Err(Error::new(ErrorKind::InvalidData, "Invalid message length"));
        }
        let msg_len = BigEndian::read_u64(buf.as_ref());
        Ok(msg_len as usize)
    }

    pub fn on_write(&mut self) -> io::Result<()> {
        let msg_op = {
            //due to this send queue maybe accessed by multi threads
            let mut queue = self._send_queue.write().unwrap();
            queue.pop()
        };

        if let Some(msg) = msg_op {
            let write_len_result = self.write_message_len(msg.clone());
            match write_len_result {
                Ok(b) => {
                    if !b {
                        //save the error message to queue, so that this message
                        //can be handled next time.
                        //it's should be ewould block
                        let mut queue = self._send_queue.write().unwrap();
                        queue.push(msg);
                        return Ok(());
                    }
                }
                Err(e) => {
                    //error happend
                    println!("write len failed: {:?}", e);
                    //in this kind of situation,
                    //may be close the connection and re-connect
                    //is a better choice
                    return Err(e);
                }
            } //end write len

            let write_result = self._stream.write(&*msg);
            match write_result {
                Ok(_) => {
                    //done , reset the flag, and 
                    self._write_next = false;
                    self._stream.flush()?;
                    return Ok(());
                }
                Err(e) => {
                    //error happened
                    if e.kind() == ErrorKind::WouldBlock {
                        //message len has been sent, only send
                        //message body next time.
                        self._write_next = true;
                        let mut queue = self._send_queue.write().unwrap();
                        queue.push(msg);
                        println!("on write , would block");
                        return Ok(());
                    }
                }
            }
        } else {
            //println!("all message has bee sended");
        }

        Ok(())
    }

    fn write_message_len(&mut self, msg: Arc<Vec<u8>>) -> io::Result<bool> {
        //means that, the remained message
        //must be sent in next round
        if self._write_next {
            return Ok(false);
        }

        let mut len_buf: [u8; 8] = [0u8; 8]; //a u64 buf
        //println!("buf.len {}", len_buf.len());

        //big endian, network order
        BigEndian::write_u64(len_buf.as_mut(), msg.len() as u64);
        let write_result = self._stream.write(&len_buf);
        match write_result {
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    //wouldblock, should be OK with false result
                    //to let this message handled in next round
                    return Ok(false);
                } else {
                    return Err(e);
                }
            }
            Ok(_) => {
                //means, write success
                return Ok(true);
            }
        }
    }

    pub fn register(&self, poll: &mut poll::Poller) -> io::Result<()> {
        poll.register_both(&self._stream, self._token)
    }

    //this message should be public to handler
    //it's multithread.
    pub fn send_message(&self, msg: Arc<Vec<u8>>) {
        let mut queue = self._send_queue.write().unwrap();
        queue.push(msg);
    }
}
