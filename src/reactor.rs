use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::rc::Rc;
use std::task::{Context, Waker};

use nix::fcntl;
use polling::{Event, Poller};
use scoped_tls::scoped_thread_local;

scoped_thread_local!(pub(crate) static REACTOR : RefCell<Reactor>);

pub(crate) struct Reactor {
    poller: Poller,
    token_to_wakers: HashMap<u64, Vec<Waker>>,
    events_buf: Vec<Event>,
}

impl Reactor {
    pub(crate) fn new() -> Self {
        Self {
            poller: Poller::new().unwrap(),
            token_to_wakers: HashMap::new(),
            events_buf: Vec::new(),
        }
    }

    fn read_key(fd: i32) -> u64 {
        fd as u64 * 2
    }

    fn write_key(fd: i32) -> u64 {
        fd as u64 * 2 + 1
    }

    pub(crate) fn add(&self, fd: &impl AsRawFd) {
        let fd = fd.as_raw_fd();
        println!("[reactor] add fd {}", fd);

        // set to non-blocking
        let flags = fcntl::OFlag::from_bits(fcntl::fcntl(fd, fcntl::F_GETFL).unwrap()).unwrap();
        let flags_nonblocking = flags | fcntl::OFlag::O_NONBLOCK;
        fcntl::fcntl(fd, fcntl::F_SETFL(flags_nonblocking)).unwrap();

        self.poller.add(fd, Event::none(fd as usize)).unwrap();
    }

    pub(crate) fn delete(&mut self, fd: &impl AsRawFd) {
        let fd = fd.as_raw_fd();
        println!("[reactor] remove fd {}", fd);

        self.token_to_wakers.remove(&(Self::read_key(fd)));
        self.token_to_wakers.remove(&(Self::write_key(fd)));

        self.poller.delete(fd).unwrap();
    }

    fn get_interest(&self, fd: RawFd) -> Event {
        let read = self.token_to_wakers.contains_key(&(fd as u64 * 2));
        let write = self.token_to_wakers.contains_key(&(fd as u64 * 2 + 1));
        let key = fd as usize;
        match (read, write) {
            (false, false) => Event::none(key),
            (true, false) => Event::readable(key),
            (false, true) => Event::writable(key),
            (true, true) => Event::all(key),
        }
    }

    pub(crate) fn wake_on_readable(&mut self, fd: &impl AsRawFd, cx: &mut Context) {
        let fd = fd.as_raw_fd();
        println!("[reactor] register interest in fd {} readable", fd);

        let key = fd as u64 * 2;
        self.token_to_wakers
            .entry(key)
            .or_default()
            .push(cx.waker().clone());

        self.poller.modify(fd, self.get_interest(fd)).unwrap()
    }

    pub(crate) fn wake_on_writable(&mut self, fd: &impl AsRawFd, cx: &mut Context) {
        let fd = fd.as_raw_fd();
        println!("[reactor] register interest in fd {} writable", fd);

        let key = fd as u64 * 2 + 1;
        self.token_to_wakers
            .entry(key)
            .or_default()
            .push(cx.waker().clone());

        self.poller.modify(fd, self.get_interest(fd)).unwrap();
    }

    pub(crate) fn wait(&mut self) {
        println!("[reactor] entering wait");
        self.poller.wait(&mut self.events_buf, None).unwrap();
        println!("[reactor] waking up with {} events", self.events_buf.len());

        for event in self.events_buf.drain(..) {
            println!("[reactor] dispatching {:?}", event);
            let fd = event.key;
            if event.readable {
                if let Some(wakers) = self.token_to_wakers.remove(&Self::read_key(fd as i32)) {
                    for waker in wakers {
                        waker.wake();
                    }
                }
            }
            if event.writable {
                if let Some(wakers) = self.token_to_wakers.remove(&Self::write_key(fd as i32)) {
                    for waker in wakers {
                        waker.wake();
                    }
                }
            }
        }

        println!("[reactor] dispatching complete");
    }
}
