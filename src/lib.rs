#![warn(unreachable_pub)]
#![allow(dead_code, unused)]

pub mod executor;
pub mod net;
mod reactor;

fn convenient() {
    tokio::net::TcpListener::bind("0.0.0.0:8000");
}
