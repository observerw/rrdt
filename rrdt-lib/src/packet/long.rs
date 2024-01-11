use super::constant::*;
use crate::{connection::CompressedParams, serializable::Serializable, TransportParams};
use bytes::{Buf, BufMut};

#[derive(Debug, Clone)]
pub struct LongHeader {}

impl LongHeader {
    pub fn new() -> Self {
        Self {}
    }
}

impl Serializable for LongHeader {
    fn decode(_data: &mut impl Buf) -> Self {
        Self {}
    }

    fn encode(self, _buf: &mut impl BufMut) {}

    fn min_len() -> usize {
        0
    }
}

pub enum LongPacket {
    Handshake(HandshakePacket),
    HandshakeDone(HandshakeDonePacket),
    Compressed(CompressedPacket),
}

impl Serializable for LongPacket {
    fn decode(data: &mut impl Buf) -> Self {
        let ty = data.get_u8();
        match ty {
            HANDSHAKE_PACKET_TYPE => Self::Handshake(HandshakePacket::decode(data)),
            HANDSHAKE_DONE_PACKET_TYPE => Self::HandshakeDone(HandshakeDonePacket::decode(data)),
            COMPRESSED_PACKET_TYPE => Self::Compressed(CompressedPacket::decode(data)),
            _ => panic!("Invalid packet type"),
        }
    }

    fn encode(self, data: &mut impl BufMut) {
        match self {
            Self::Handshake(packet) => {
                data.put_u8(HANDSHAKE_PACKET_TYPE);
                packet.encode(data);
            }
            Self::HandshakeDone(packet) => {
                data.put_u8(HANDSHAKE_DONE_PACKET_TYPE);
                packet.encode(data);
            }
            Self::Compressed(packet) => {
                data.put_u8(COMPRESSED_PACKET_TYPE);
                packet.encode(data);
            }
        }
    }

    fn min_len() -> usize {
        std::mem::size_of::<u8>()
    }

    fn len(&self) -> usize {
        Self::min_len()
            + match self {
                Self::Handshake(packet) => packet.len(),
                Self::HandshakeDone(packet) => packet.len(),
                Self::Compressed(packet) => packet.len(),
            }
    }
}

pub struct HandshakePacket {
    header: LongHeader,
    params: TransportParams,
}

impl HandshakePacket {
    pub fn new(params: TransportParams) -> Self {
        let header = LongHeader::new();
        Self { header, params }
    }

    pub fn into_params(self) -> TransportParams {
        self.params
    }
}

impl Serializable for HandshakePacket {
    fn decode(data: &mut impl Buf) -> Self {
        let header = LongHeader::decode(data);
        let params = TransportParams::decode(data);
        Self { header, params }
    }

    fn encode(self, data: &mut impl BufMut) {
        self.header.encode(data);
        self.params.encode(data);
    }

    fn min_len() -> usize {
        LongHeader::min_len() + TransportParams::min_len()
    }
}

pub struct CompressedPacket {
    header: LongHeader,
    params: CompressedParams,
}

impl CompressedPacket {
    pub fn new(params: CompressedParams) -> Self {
        let header = LongHeader::new();
        Self { header, params }
    }

    pub fn into_params(self) -> CompressedParams {
        self.params
    }
}

impl Serializable for CompressedPacket {
    fn decode(data: &mut impl Buf) -> Self {
        let header = LongHeader::decode(data);
        let params = CompressedParams::decode(data);
        Self { header, params }
    }

    fn encode(self, data: &mut impl BufMut) {
        self.header.encode(data);
        self.params.encode(data);
    }

    fn min_len() -> usize {
        LongHeader::min_len() + CompressedParams::min_len()
    }
}

pub struct HandshakeDonePacket {
    header: LongHeader,
}

impl HandshakeDonePacket {
    pub fn new() -> Self {
        let header = LongHeader::new();
        Self { header }
    }
}

impl Serializable for HandshakeDonePacket {
    fn decode(data: &mut impl Buf) -> Self {
        let header = LongHeader::decode(data);
        Self { header }
    }

    fn encode(self, data: &mut impl BufMut) {
        self.header.encode(data);
    }

    fn min_len() -> usize {
        LongHeader::min_len()
    }
}
