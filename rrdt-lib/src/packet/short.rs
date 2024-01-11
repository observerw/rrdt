use super::constant::*;
use crate::{
    frame::{Frame, FrameMeta},
    serializable::Serializable,
    types::PacketNum,
};
use bytes::{Buf, BufMut};
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct Packet {
    header: Header,
    frames: Vec<Frame>,
}

impl Packet {
    pub fn new(packet_num: PacketNum) -> Self {
        let header = Header::new(packet_num);
        Self {
            header,
            frames: vec![],
        }
    }

    pub fn with_header(header: Header) -> Self {
        Self {
            header,
            frames: vec![],
        }
    }

    pub fn with_frames(self, frames: Vec<Frame>) -> Self {
        Self { frames, ..self }
    }

    pub fn packet_num(&self) -> PacketNum {
        self.header.packet_num()
    }

    pub fn into_frames(self) -> Vec<Frame> {
        self.frames
    }

    pub fn push(&mut self, frame: Frame) {
        self.frames.push(frame)
    }

    pub fn meta(&self, sent: Instant) -> PacketMeta {
        let packet_num = self.packet_num();
        let bytes = self.len() as u64;
        let frame_meta = self
            .frames
            .iter()
            .filter_map(|frame| frame.meta())
            .collect();
        let is_ack_eliciting = self.is_ack_eliciting();

        PacketMeta {
            packet_num,
            frame_meta,
            sent,
            bytes,
            is_ack_eliciting,
        }
    }

    /// packet中最多还可以容纳多少字节
    pub fn remaining(&self) -> usize {
        MAX_PACKET_SIZE - self.len()
    }

    /// 包含非ACK、PADDING和CONNECTION_CLOSE帧的packet是ack eliciting的
    pub fn is_ack_eliciting(&self) -> bool {
        self.frames
            .iter()
            .any(|frame| !matches!(frame, Frame::Ack(_)))
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

impl Serializable for Packet {
    fn decode(data: &mut impl Buf) -> Self {
        let header = Header::decode(data);

        let mut frames = Vec::new();
        // 由于packet中并没有frame的数量信息，所以这里只能将data中剩余的全部数据认为是frame
        while data.has_remaining() {
            let frame = Frame::decode(data);
            frames.push(frame);
        }

        Self::with_header(header).with_frames(frames)
    }

    fn encode(self, data: &mut impl BufMut) {
        self.header.encode(data);
        for frame in self.frames {
            frame.encode(data);
        }
    }

    fn min_len() -> usize {
        Header::min_len()
    }

    fn len(&self) -> usize {
        Self::min_len() + self.frames.iter().map(|frame| frame.len()).sum::<usize>()
    }
}

#[derive(Clone, Debug)]
pub struct PacketMeta {
    pub packet_num: PacketNum,
    /// packet中包含的stream frame的meta信息
    pub frame_meta: Vec<FrameMeta>,
    /// packet的发送时间
    pub sent: Instant,
    /// packet的大小
    pub bytes: u64,
    /// packet是否是ack eliciting的
    pub is_ack_eliciting: bool,
}

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
