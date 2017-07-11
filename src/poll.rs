
use mio::{Events, Poll, Token, Ready, PollOpt, Evented};
use std::io;

pub struct Poller {
    _poll: Poll,
}

impl Poller {
    pub fn new() -> io::Result<Poller> {
        let poll = Poll::new()?;

        return Ok(Poller { _poll: poll });
    }

    pub fn poll_once(&self, events: &mut Events) -> io::Result<usize> {
        //println!("poll: {:?} {:?}", self._poll, events);
        self._poll.poll(events, None)
    }

    pub fn register_read<E: ?Sized>(&self, handle: &E, token: Token) -> io::Result<()>
    where
        E: Evented,
    {
        self.register(handle, token, Ready::readable(), PollOpt::edge())
    }

    pub fn register_both<E: ?Sized>(&self, handle: &E, token: Token) -> io::Result<()>
    where
        E: Evented,
    {
        let mut ready = Ready::readable();
        ready.insert(Ready::writable());
        self.register(handle, token, ready, PollOpt::edge())
    }

    pub fn register<E: ?Sized>(
        &self,
        handle: &E,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()>
    where
        E: Evented,
    {
        let result = self._poll.register(handle, token, interest, opts)?;
        println!("regist: {:?}, {:?}, {:?}", token, interest, opts);
        Ok(result)
    }
}

/*
impl Deref<Poll> for Poller {

} */
