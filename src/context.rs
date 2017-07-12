
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{Ordering, AtomicBool};
use std::io;
use std::io::{Error, ErrorKind};
use mio::{Token, Events};
use mio::net::{TcpListener, TcpStream};
use slab;
use connection;
use poll;
use server::SERVERTOKEN;

#[derive(Clone)]
pub struct ServerContext {
    pub _listener: Arc<RwLock<TcpListener>>,
    pub _conns: Arc<RwLock<slab::Slab<connection::Connection, Token>>>,
    pub _poller: Arc<RwLock<poll::Poller>>,

    _is_registed: Arc<AtomicBool>,
}

impl ServerContext {
    pub fn new(addr: &str, max_clients: usize) -> Option<ServerContext> {
        if max_clients >= SERVERTOKEN.into() {
            println!("too many clients, should less than 1_000");
            return None;
        }
        let poller_result = poll::Poller::new();
        let poller = match poller_result {
            Ok(p) => p,
            Err(_) => {
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
                                _conns:
                                    Arc::new(RwLock::new(slab::Slab::with_capacity(max_clients))),
                                _poller: Arc::new(RwLock::new(poller)),
                                _is_registed: Arc::new(AtomicBool::new(false)),
                            });
            } else {
                println!("bind failed,  port used?");
            }
        }

        println!("address format [ip:port]");
        None
    }

    pub fn register_read(&self, token: Token) -> io::Result<()> {
        if self._is_registed.load(Ordering::SeqCst) {
            return Ok(());
        }

        let poller = self._poller.read().unwrap();
        let listener_guard = self._listener.read().unwrap();
        self._is_registed.store(true, Ordering::SeqCst);
        poller.register_read(&*listener_guard, token)
    }

    pub fn poll_once(&self, events: &mut Events) -> io::Result<usize> {
        let poller = self._poller.read().unwrap();
        poller.poll_once(events)
    }

    pub fn remove_client(&self, token: Token) {
        let mut clients = self._conns.write().unwrap();
        clients.remove(token);
    }

    pub fn available_token(&self, client: TcpStream) -> Option<Token> {
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

    pub fn register_token(&self, token: Token) -> io::Result<()> {
        let clients = self._conns.read().unwrap();
        let mut poller = self._poller.write().unwrap();
        if let Some(c) = clients.get(token) {
            c.register(&mut poller)?;
            return Ok(());
        }

        Err(Error::new(ErrorKind::InvalidInput, "Invalid token"))
    }

    #[allow(dead_code)]
    pub fn send_message_to_client(&self, token: Token, msg: Arc<Vec<u8>>) -> io::Result<()> {
        let conns = self._conns.read().unwrap();
        let client_op = conns.get(token);
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
