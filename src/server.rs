
use std::sync::{Arc};
use std::io;
use std::io::{Error, ErrorKind};
use mio::{Event, Events, Token, Evented};
use mio::net::TcpListener;
use mio::unix::UnixReady;
use context::Context;
use serialize;
use poll;

//must be less than this
pub const SERVERTOKEN: Token = Token(1000_000);

//#[derive(Clone)]
pub struct Server {
    _token: Token,
    _events: Events,

    _listener:TcpListener,
    _poller:poll::Poller,
}

/*
impl<T: serialize::MessageHandler + Sized> Clone for Server<T> {
    fn clone(&self) -> Self {
        Server {
            _token: self._token,
            _events: Events::with_capacity(self._events.capacity()),
            _handle: self._handle.clone(),
            _listener:self._listener.try_clone(),
            _poller:poll::Poller.clone(),
        }
    }
} */

impl Server {
    pub fn new(listener:TcpListener) -> Option<Self> {
        let p = poll::Poller::new();
        if let Ok(poll) = p {
            return Some(Server {
                _token: SERVERTOKEN,
                _events: Events::with_capacity(1024),
                _listener:listener,
                _poller:poll,
            });
        }
        None
    }

    pub fn poll_once(&mut self) -> io::Result<usize> {
        self._poller.poll_once(&mut self._events)
    }

    pub fn unregister_token<T>(&self, ctx:&Context<T>, token:Token)->io::Result<()> 
        where T:serialize::MessageHandler + Sized {
        
        let clients = ctx._conns.read().unwrap();
        let client = clients.get(token);
        if let Some(c) = client {
            return self._poller.deregister(&c._stream);
        }
        
        Ok(())
    }

    #[allow(dead_code)]
    pub fn unregister<E: ?Sized>(&self, handle:&E)->io::Result<()> 
        where E:Evented {
        self._poller.deregister(handle)
    }

    pub fn register_read(&self, token: Token) -> io::Result<()> {    
        //Listener token will be registered into every poller instance
        //in every server=====, in case of thread panic
        self._poller.register_read(&self._listener, token)
    }

    pub fn run<T>(&mut self, ctx: &Context<T>)  -> io::Result<()>
        where T: serialize::MessageHandler + Sized {
        
        println!("--------start to register {:?}", self._token);
        self.register_read(self._token)?;
        println!("+++++run {:?} {:?}", self._token, self._listener);
        loop {
            println!("start to poll");
            let size = self.poll_once()?;
            println!("poll size{}", size);

            for i in 0..size {
                let event_op = self._events.get(i);
                if let Some(event) = event_op {
                    self.on_event(ctx, &event);
                } else {
                    println!("errror event");
                    break;
                }
            }//end for?
        }
    }

    pub fn register_token<T>(&mut self, ctx:&Context<T>, token: Token) -> io::Result<()> 
        where T: serialize::MessageHandler + Sized {
        let clients = ctx._conns.read().unwrap();
        
        if let Some(c) = clients.get(token) {
            c.register(&mut self._poller)?;
            return Ok(());
        }

        Err(Error::new(ErrorKind::InvalidInput, "Invalid token"))
    }

    fn on_event<T>(&mut self, ctx: &Context<T>, event: &Event)
        where T : serialize::MessageHandler + Sized {
        let ready = UnixReady::from(event.readiness());
        let token = event.token();
        if ready.is_error() {
            if let Err(e) = self.unregister_token(ctx, token) {
                println!("-----------------------{:?}", e);
            }

            ctx.remove_client(token);
            println!("error event recv");
            return;
        }

        let mut vec: Vec<Token> = vec![];
        if ready.is_readable() {
            if token == self._token {
                println!("new client connected");
                self.on_accept(ctx);
            } else {
                println!("forward read, token={:?}", token);
                match self.dispatch_read(token, ctx) {
                    Err(_) => {
                        vec.push(token);
                    }
                    Ok(_) => {}
                }
            }
        } //end

        if ready.is_writable() {
            let mut conns = ctx._conns.write().unwrap();
            let client_op = conns.get_mut(token);
            if let Some(mut c) = client_op {
                println!("client write event, token={:?}", token);
                c.on_write()
                    .unwrap_or_else(|_| { vec.push(c.get_token()); });
            }
        }

        for token in vec {
            if let Err(e) = self.unregister_token(ctx, token) {
                println!("-----------------------{:?}", e);
            }

            ctx.remove_client(token);
        }
    }

    fn on_accept<T>(&mut self, ctx: &Context<T>)
        where T : serialize::MessageHandler + Sized {
        loop {
            let accept_result = self._listener.accept();
            //println!("get one client");
            let client = match accept_result {
                Ok((c, _)) => c,
                Err(e) => {
                    if e.kind() != ErrorKind::WouldBlock {
                        println!("accept error");
                    }
                    //println!("accept wuld block, {:?}", e);
                    return;
                }
            };
            let token = ctx.available_token(client);
            if let Some(t) = token {
                println!("client added:......");
                self.register_token(ctx, t).expect("register client failed");
            } else {
                println!("no available token found");
            }
        }
    }

    fn dispatch_read<T>(&mut self, token: Token, ctx: &Context<T>) -> io::Result<()>
        where T : serialize::MessageHandler + Sized {
        let mut conns = ctx._conns.write().unwrap();
        let client_op = conns.get_mut(token);
        let client = match client_op {
            Some(expr) => expr,
            None => {
                println!("no client got:{:?}", token);
                return Err(Error::new(ErrorKind::InvalidData, "invlid token"));
            }
        };

        loop {
            let read_result = client.on_read();
            if let Ok(read_op) = read_result {
                if let Some(message) = read_op {
                    let rc_message = Arc::new(message);
                    println!("client send message start..");
                    // Queue up a write for all connected clients.
                    let handler = ctx._handle.read().unwrap();
                    handler.on_message_received(&client, &rc_message)?
                    //client.send_message(rc_message.clone());
                } else {
                    println!("forward read: no message got");
                    break;
                }
            } else {
                println!("forward read: read failed");
                return Err(Error::new(ErrorKind::InvalidData, "read failed"));
            }
        }
        Ok(())
    }
}
