use tokio::net::TcpStream;
use tokio_serde::{formats::SymmetricalJson, SymmetricallyFramed};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::dispatcher::PeerMessage;

type TcpFrame = Framed<TcpStream, LengthDelimitedCodec>;
// XXX: maybe rename to PeerMessageFramed
pub type MessageFramed = SymmetricallyFramed<TcpFrame, PeerMessage, SymmetricalJson<PeerMessage>>;

pub fn create_encoder(stream: TcpStream) -> MessageFramed {
    let tcp_frame = TcpFrame::new(stream, LengthDelimitedCodec::new());

    let json_frame = MessageFramed::new(tcp_frame, SymmetricalJson::<PeerMessage>::default());

    return json_frame;
}
