use tokio_serde::{SymmetricallyFramed, formats::SymmetricalJson};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tokio::net::TcpStream;
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::address_book::{PeerMessage};

type TcpFrame = Framed<TcpStream, LengthDelimitedCodec>;
pub type MessageFramed = SymmetricallyFramed<TcpFrame, PeerMessage, SymmetricalJson<PeerMessage>>;

pub struct Stream {
    inner: MessageFramed,
}
impl PartialEq for Stream {
    fn eq(&self, other: &Self) -> bool {
        // XXX hack
        return true;
    }
}

impl Eq for Stream {}

impl Stream {
    pub fn new(inner: TcpStream) -> Stream {
        let tcp_frame = TcpFrame::new(
            inner,
            LengthDelimitedCodec::new());

        let json_frame = MessageFramed::new(
            tcp_frame,
            SymmetricalJson::<PeerMessage>::default(),
        );
        return Stream {
            inner: json_frame,
        }
    }
    pub fn from_framed(inner: MessageFramed) -> Stream {
        return Stream {
            inner
        }
    }

    pub fn get_framed(self) -> MessageFramed {
        return self.inner;
    }
}

impl fmt::Display for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Encoding Stream")
    }
}

impl fmt::Debug for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Encoding Stream")
    }
}

// XXX: use a stupid hash which will cause collisions
impl Hash for Stream{
    fn hash<H: Hasher>(&self, state: &mut H) {
        "foobar".to_string().hash(state);
    }
}
