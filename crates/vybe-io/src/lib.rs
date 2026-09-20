//! # vybe-io
//!
//! vybe's doors to the outside. Today: **OSC over UDP**, in and out — the
//! protocol a sensor board, TouchDesigner, and vybe's own remote all speak.
//! (GPIO joins it when a work runs on a Pi.)
//!
//! This crate knows sockets and nothing of pictures. It plugs into the engine
//! from outside, through `vybe::input::Binding` — see `vybe-remote` for the
//! glue. `rosc` does the wire format and never leaks past this file.

use std::io;
use std::net::{SocketAddr, UdpSocket};

use rosc::{OscMessage, OscPacket, OscType};

/// One OSC argument, narrowed to what vybe's protocols carry.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Int(i32),
    Float(f32),
    Str(String),
}

impl Arg {
    /// The argument as a number (an int widens; a string is not one).
    pub fn number(&self) -> Option<f32> {
        match self {
            Arg::Int(i) => Some(*i as f32),
            Arg::Float(f) => Some(*f),
            Arg::Str(_) => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Arg::Str(s) => Some(s),
            _ => None,
        }
    }
}

impl From<f32> for Arg {
    fn from(f: f32) -> Self {
        Arg::Float(f)
    }
}

impl From<i32> for Arg {
    fn from(i: i32) -> Self {
        Arg::Int(i)
    }
}

impl From<&str> for Arg {
    fn from(s: &str) -> Self {
        Arg::Str(s.to_owned())
    }
}

/// `/address arg arg …`
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub address: String,
    pub args: Vec<Arg>,
}

impl Message {
    pub fn new(address: &str, args: impl IntoIterator<Item = Arg>) -> Self {
        Self {
            address: address.to_owned(),
            args: args.into_iter().collect(),
        }
    }

    /// The `i`-th argument as a number.
    pub fn number(&self, i: usize) -> Option<f32> {
        self.args.get(i)?.number()
    }
}

/// A UDP socket that speaks OSC. Never blocks: [`Osc::recv`] returns what has
/// arrived, which is how a render loop wants it.
pub struct Osc {
    socket: UdpSocket,
    /// Bundled messages wait here until asked for, one per `recv`.
    pending: Vec<(Message, SocketAddr)>,
}

impl Osc {
    /// Listens on `port`, on every interface.
    pub fn bind(port: u16) -> io::Result<Self> {
        Self::from_socket(UdpSocket::bind(("0.0.0.0", port))?)
    }

    /// A socket on whatever port is free — for a side that speaks first.
    pub fn open() -> io::Result<Self> {
        Self::bind(0)
    }

    fn from_socket(socket: UdpSocket) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            pending: Vec::new(),
        })
    }

    pub fn port(&self) -> u16 {
        self.socket.local_addr().map_or(0, |a| a.port())
    }

    /// The next message that has arrived and who sent it, if any. Malformed
    /// packets are dropped: a render loop must outlive a confused sender.
    pub fn recv(&mut self) -> Option<(Message, SocketAddr)> {
        let mut buf = [0u8; rosc::decoder::MTU];
        while self.pending.is_empty() {
            let (len, from) = self.socket.recv_from(&mut buf).ok()?;
            if let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..len]) {
                unpack(packet, from, &mut self.pending);
                // Oldest first out of a bundle.
                self.pending.reverse();
            }
        }
        self.pending.pop()
    }

    pub fn send(&self, to: SocketAddr, message: &Message) -> io::Result<()> {
        let packet = OscPacket::Message(OscMessage {
            addr: message.address.clone(),
            args: message
                .args
                .iter()
                .map(|arg| match arg {
                    Arg::Int(i) => OscType::Int(*i),
                    Arg::Float(f) => OscType::Float(*f),
                    Arg::Str(s) => OscType::String(s.clone()),
                })
                .collect(),
        });
        let bytes = rosc::encoder::encode(&packet)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        self.socket.send_to(&bytes, to).map(|_| ())
    }
}

/// Flattens a packet (bundles nest) into plain messages.
fn unpack(packet: OscPacket, from: SocketAddr, out: &mut Vec<(Message, SocketAddr)>) {
    match packet {
        OscPacket::Message(message) => out.push((
            Message {
                address: message.addr,
                args: message
                    .args
                    .into_iter()
                    .filter_map(|arg| match arg {
                        OscType::Int(i) => Some(Arg::Int(i)),
                        OscType::Long(i) => Some(Arg::Int(i as i32)),
                        OscType::Float(f) => Some(Arg::Float(f)),
                        OscType::Double(f) => Some(Arg::Float(f as f32)),
                        OscType::Bool(b) => Some(Arg::Int(i32::from(b))),
                        OscType::String(s) => Some(Arg::Str(s)),
                        _ => None,
                    })
                    .collect(),
            },
            from,
        )),
        OscPacket::Bundle(bundle) => {
            for inner in bundle.content {
                unpack(inner, from, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Polls until a message arrives (UDP on loopback is fast, not instant).
    fn wait(osc: &mut Osc) -> (Message, SocketAddr) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(got) = osc.recv() {
                return got;
            }
            assert!(Instant::now() < deadline, "no message arrived");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn a_message_crosses_loopback_and_can_be_answered() {
        let mut face = Osc::open().unwrap();
        let mut remote = Osc::open().unwrap();
        let face_addr: SocketAddr = ([127, 0, 0, 1], face.port()).into();

        let corner = Message::new(
            "/keystone/corner",
            [2.into(), 0.98f32.into(), 1.0f32.into()],
        );
        remote.send(face_addr, &corner).unwrap();
        let (got, from) = wait(&mut face);
        assert_eq!(got, corner);
        assert_eq!(got.number(0), Some(2.0));

        face.send(from, &Message::new("/keystone/saved", ["ok".into()]))
            .unwrap();
        let (reply, _) = wait(&mut remote);
        assert_eq!(reply.args[0].text(), Some("ok"));
    }

    #[test]
    fn an_empty_socket_returns_at_once() {
        assert!(Osc::open().unwrap().recv().is_none());
    }
}
