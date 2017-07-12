
use std::sync::{Arc, RwLock};
use std::io;
use std::io::{Error, ErrorKind};
use mio::{Event, Events, Token};
use context::ServerContext;
use serialize;

//must be less than this
pub const SERVERTOKEN: Token = Token(1000);

//#[derive(Clone)]
pub struct Server<T: serialize::MessageHandler + Sized> {
    _token: Token,
    _events: Events,

    //the refcell used betten than raw trait
    //it can callback the mut function when needed
    _handle: Arc<RwLock<T>>,
}

impl<T: serialize::MessageHandler + Sized> Clone for Server<T> {
    fn clone(&self) -> Self {
        Server {
            _token: self._token,
            _events: Events::with_capacity(self._events.capacity()),
            _handle: self._handle.clone(),
        }
    }
}

impl<T: serialize::MessageHandler + Sized> Server<T> {
    pub fn new(handler: T) -> Server<T> {
        let ptr = Arc::new(RwLock::new(handler));

        return Server {
                   _token: SERVERTOKEN,
                   _events: Events::with_capacity(1024),
                   _handle: ptr,
               };
    }

    pub fn run(&mut self, ctx: &ServerContext) -> io::Result<()> {
        ctx.register_read(self._token)?;
        //println!("run {:?} {:?}", self._token, self._listener);
        loop {
            println!("start to poll");
            let size = ctx.poll_once(&mut self._events)?;
            println!("poll size{}", size);

            for i in 0..size {
                let event_op = self._events.get(i);
                if let Some(event) = event_op {
                    self.on_event(ctx, &event);
                } else {
                    println!("errror event");
                    break;
                }
            }
        }
    }

    fn on_event(&mut self, ctx: &ServerContext, event: &Event) {
        let ready = event.readiness();
        let token = event.token();
        if ready.is_error() {
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
                match self.forward_readable(token, ctx) {
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
            ctx.remove_client(token);
        }
    }

    fn on_accept(&mut self, ctx: &ServerContext) {
        loop {
            let accept_result = ctx._listener.read().unwrap().accept();
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
                ctx.register_token(t).expect("register client failed");
            } else {
                println!("no available token found");
            }
        }
    }

    fn forward_readable(&mut self, token: Token, ctx: &ServerContext) -> io::Result<()> {
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
                    let handler = self._handle.read().unwrap();
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
