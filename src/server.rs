
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{Ordering, AtomicBool};
use std::io;
use std::io::{Error, ErrorKind};
use mio::{Event, Events, Token};
use mio::net::{TcpListener, TcpStream};
use mio::unix::UnixReady;
use slab;
use connection;
use poll;

//must be less than this
const SERVERTOKEN: Token = Token(1000);

#[derive(Clone)]
pub struct ServerContext {
    _listener: Arc<RwLock<TcpListener>>,
    _conns: Arc<RwLock<slab::Slab<connection::Connection, Token>>>,
    _poller:Arc<RwLock<poll::Poller>>,

    _is_registed:Arc<AtomicBool>,
}

impl ServerContext {
    pub fn new(addr: &str, max_clients: usize) -> Option<ServerContext> {
        if max_clients >= SERVERTOKEN.into() {
            println!("too many clients, should less than 1_000");
            return None;
        }
        let poller_result = poll::Poller::new();
        let poller = match poller_result {
            Ok(p)=>{p},
            Err(_)=>{
                return None;
            }
        };

        let address = addr.parse::<SocketAddr>();
        if let Ok(r) = address {
            let listener = TcpListener::bind(&r);
            println!("binded {:?}", r);
            if let Ok(l) = listener {
                return Some(ServerContext {
                    _listener: Arc::new(RwLock::new(l)),
                    _conns: Arc::new(RwLock::new(slab::Slab::with_capacity(max_clients))),
                    _poller:Arc::new(RwLock::new(poller)),
                    _is_registed:Arc::new(AtomicBool::new(false)),
                });
            } else {
                println!("bind failed,  port used?");
            }
        }

        println!("address format [ip:port]");
        None
    }

    pub fn register_read(&self, token:Token)->io::Result<()> {
        if self._is_registed.load(Ordering::SeqCst) {
            return Ok(());
        }

        let poller = self._poller.read().unwrap();
        let listener_guard = self._listener.read().unwrap();
        self._is_registed.store(true, Ordering::SeqCst);
        poller.register_read(&*listener_guard, token)
    }

    pub fn poll_once(&self, events:&mut Events) ->io::Result<usize> {
        let poller = self._poller.read().unwrap();
        poller.poll_once(events)
    }

    pub fn remove_client(&self, token:Token) {
        let mut clients = self._conns.write().unwrap();
        clients.remove(token);
    }

    fn available_token(& self, client: TcpStream) -> Option<Token> {
        let mut conns = self._conns.write().unwrap();
        let entry_op = conns.vacant_entry();
        let token = match entry_op {
            Some(e) => {
                let connection = connection::Connection::new(client, e.index());
                e.insert(connection).index()
            }
            None => {
                println!("no empty entry for new clients");
                return None;
            }
        };

        Some(token)
    }

    fn register_token(&self, token:Token)->io::Result<()>{
        let clients = self._conns.read().unwrap();
        let mut poller = self._poller.write().unwrap();
        if let Some(c) = clients.get(token) {
            c.register(&mut poller)?;
            return Ok(());
        }

        Err(Error::new(ErrorKind::InvalidInput, "Invalid token"))
    }

    #[allow(dead_code)]
    pub fn send_message_to_client(& self,  token: Token,  msg:Arc<Vec<u8>>) -> io::Result<()> {
        let mut conns = self._conns.write().unwrap();
        let client_op = conns.get_mut(token);
        let client = match client_op {
            Some(expr) => expr,
            None => {
                println!("no client got:{:?}", token);
                return Err(Error::new(ErrorKind::InvalidData, "invlid token"));
            }
        };

        client.send_message(msg.clone());
        Ok(())
    } //end send?

}

//#[derive(Clone)]
pub struct Server {
    _token: Token,
    _events: Events,
}

impl Clone for Server {
    fn clone(&self) -> Self {
        Server {
            _token:self._token,
            _events:Events::with_capacity(self._events.capacity()),
        }    
    }
}

impl Server {
    pub fn new() -> Server {
            return Server {
                _token:SERVERTOKEN,
                _events:Events::with_capacity(1024),
            };
    }

    pub fn run(&mut self, ctx : &ServerContext) -> io::Result<()> {
        ctx.register_read( self._token)?;
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

    fn on_event(&mut self,  ctx : &ServerContext, event: &Event) {
        let ready = UnixReady::from(event.readiness());
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
            if let Some(mut c) = client_op{
                println!("client write event, token={:?}", token);
                c.on_write().unwrap_or_else(
                    |_| { vec.push(c.get_token()); }
                );
            }
        } 

        for token in vec {
            ctx.remove_client(token);
        }
    }

    fn on_accept(&mut self, ctx : &ServerContext) {
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

    fn forward_readable(&mut self, token: Token,  ctx : &ServerContext) -> io::Result<()> {
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
                    client.send_message(rc_message.clone());
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
