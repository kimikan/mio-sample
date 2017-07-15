
/*written by kimikan, 2017-7-12*/
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::io;
use std::io::{Error, ErrorKind};
use mio::{Token};
use mio::net::{TcpListener, TcpStream};
use slab;
use connection;
use serialize;

/* only one tcplistener */
pub fn bind(addr:&str)->io::Result<TcpListener> {
    let address = addr.parse::<SocketAddr>();
    if let Ok(r) = address {
        return TcpListener::bind(&r);
    }

    Err(Error::new(ErrorKind::InvalidInput, "Invalid input"))
}

//#[derive(Clone)]
pub struct Context<T: serialize::MessageHandler + Sized> {
    pub _conns: Arc<RwLock<slab::Slab<connection::Connection, Token>>>,
    //the refcell used betten than raw trait
    //it can callback the mut function when needed
    pub _handle: Arc<RwLock<T>>,

    pub _capacity:usize,
}

impl<T: serialize::MessageHandler+Sized> Clone for Context<T> {
    fn clone(&self) -> Self {
        Context {
            _conns:Arc::new(RwLock::new(slab::Slab::with_capacity(self._capacity))),
            _handle: self._handle.clone(),
            _capacity:self._capacity,
        }
    }
}

impl<T: serialize::MessageHandler + Sized> Context<T> {
    pub fn new(handle:T, max_clients:usize) -> Self {
        Context {
            _conns: Arc::new(RwLock::new(slab::Slab::with_capacity(max_clients))),
            _handle:Arc::new(RwLock::new(handle)),
            _capacity:max_clients,
        }
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
