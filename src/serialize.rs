
use std::sync::Arc;
use std::io;

use connection::Connection;

//will implement later
//to integrete with the codec module
//or just self-define it.
pub trait MessageCodec {
    fn encode(&self, _: Arc<Vec<u8>>);

    fn decode(&self, _: Arc<Vec<u8>>);
}

//message handler, is a interface to
//process the coming message
pub trait MessageHandler {
    //this function defines that the callback function
    //while there is new message received
    //general logic, is parse it, then use context.send_message(token, message);
    //to response the client.
    fn on_message_received(&self, _: &Connection, _: &Arc<Vec<u8>>) -> io::Result<()>;
}
