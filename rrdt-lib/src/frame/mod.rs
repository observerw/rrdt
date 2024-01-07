use self::{
    ack::AckFrame,
    constant::*,
    handshake::HandshakeFrame,
    stream::{MaxStreamDataFrame, MaxStreamDataMeta, StreamDataFrame, StreamDataMeta},
};
use crate::{serializable::Serializable, types::StreamId};
use bytes::{Buf, BufMut};

pub mod ack;
mod constant;
pub mod handshake;
pub mod stream;

#[derive(Debug, Clone)]
pub enum Frame {
    Handshake(HandshakeFrame),
    Ack(AckFrame),
    Stream(StreamDataFrame),
    MaxStreamData(MaxStreamDataFrame),
}

impl Frame {
    pub fn meta(&self) -> Option<FrameMeta> {
        match self {
            Frame::Stream(frame) => Some(FrameMeta::Stream(frame.meta())),
            Frame::MaxStreamData(frame) => Some(FrameMeta::MaxStreamData(frame.meta())),
            _ => None,
        }
    }
}

impl Serializable for Frame {
    fn decode(data: &mut impl Buf) -> Self {
        let ty = data.get_u8();
        match ty {
            HANDSHAKE_TYPE => Frame::Handshake(HandshakeFrame::decode(data)),
            ACK_TYPE => Frame::Ack(AckFrame::decode(data)),
            STREAM_TYPE => Frame::Stream(StreamDataFrame::decode(data)),
            STREAM_FIN_TYPE => Frame::Stream(StreamDataFrame::decode(data).with_fin()),
            MAX_STREAM_DATA_TYPE => Frame::MaxStreamData(MaxStreamDataFrame::decode(data)),
            _ => panic!("unknown frame type: {}", ty),
        }
    }

    fn encode(self, data: &mut impl BufMut) {
        match self {
            Frame::Handshake(frame) => {
                data.put_u8(HANDSHAKE_TYPE);
                frame.encode(data);
            }
            Frame::Stream(frame) => {
                data.put_u8(frame.ty());
                frame.encode(data);
            }
            Frame::Ack(frame) => {
                data.put_u8(ACK_TYPE);
                frame.encode(data);
            }
            Frame::MaxStreamData(frame) => {
                data.put_u8(MAX_STREAM_DATA_TYPE);
                frame.encode(data);
            }
        }
    }

    fn min_len() -> usize {
        // type
        std::mem::size_of::<u8>()
    }

    fn len(&self) -> usize {
        match self {
            Frame::Handshake(frame) => frame.len(),
            Frame::Stream(frame) => frame.len(),
            Frame::Ack(frame) => frame.len(),
            Frame::MaxStreamData(frame) => frame.len(),
        }
    }
}

/// 需要重传的frame均有一个对应的meta，用于在重传时更新inflight信息
#[derive(Clone, Debug)]
pub enum FrameMeta {
    Stream(StreamDataMeta),
    MaxStreamData(MaxStreamDataMeta),
}

#[derive(Debug)]
pub enum StreamFrame {
    Data(StreamDataFrame),
    MaxData(MaxStreamDataFrame),
}

impl StreamFrame {
    pub fn id(&self) -> StreamId {
        match self {
            StreamFrame::Data(frame) => frame.id,
            StreamFrame::MaxData(frame) => frame.id,
        }
    }
}
