use super::PacketNum;
use crate::serializable::Serializable;
use bytes::{Buf, BufMut};

#[derive(Debug, Clone)]
pub struct Header {
    packet_num: PacketNum,
}

impl Header {
    pub fn new(packet_num: PacketNum) -> Self {
        Self { packet_num }
    }

    pub fn packet_num(&self) -> PacketNum {
        self.packet_num
    }

    pub(crate) fn set_packet_num(&mut self, packet_num: PacketNum) {
        self.packet_num = packet_num;
    }
}

impl Serializable for Header {
    fn decode(data: &mut impl Buf) -> Self {
        let packet_num = data.get_u64();

        Self { packet_num }
    }

    fn encode(self, buf: &mut impl BufMut) {
        buf.put_u64(self.packet_num);
    }

    fn min_len() -> usize {
        // packet_num
        std::mem::size_of::<u64>()
    }
}

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
