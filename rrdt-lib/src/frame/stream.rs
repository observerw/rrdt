use super::constant::*;
use crate::{serializable::Serializable, types::StreamId};
use bytes::{Buf, BufMut, Bytes};
use std::fmt::Debug;
use std::{fmt::Formatter, ops::Range};

#[derive(Clone)]
pub struct StreamDataFrame {
    pub id: StreamId,
    pub offset: u64,
    pub data: Bytes,
    pub fin: bool,
}

impl StreamDataFrame {
    pub fn with_fin(mut self) -> Self {
        self.fin = true;
        self
    }

    /// stream frame可能有多种类型
    pub fn ty(&self) -> u8 {
        if self.fin {
            STREAM_FIN_TYPE
        } else {
            STREAM_TYPE
        }
    }

    /// 将当前frame分成两个frame，保证分割出去的frame的大小恰好为`at`
    ///
    /// 被分割出去的frame必定不是fin frame
    pub fn split_to(&mut self, at: usize) -> Self {
        assert!(at > Self::min_len());
        assert!(at < self.len());

        let data_at = at - Self::min_len();

        let result = StreamDataFrame {
            id: self.id,
            offset: self.offset,
            data: self.data.split_to(data_at),
            fin: false,
        };

        self.offset += data_at as u64;

        result
    }

    pub fn meta(&self) -> StreamDataMeta {
        StreamDataMeta {
            id: self.id,
            range: self.offset..self.offset + self.data.len() as u64,
        }
    }
}

impl Serializable for StreamDataFrame {
    fn decode(data: &mut impl Buf) -> Self {
        let id = data.get_u16();
        let offset = data.get_u64();
        let length = data.get_u64();
        let data = data.copy_to_bytes(length as usize);

        Self {
            id,
            offset,
            data,
            fin: false,
        }
    }

    fn encode(self, data: &mut impl BufMut) {
        data.put_u16(self.id);
        data.put_u64(self.offset);
        data.put_u64(self.data.len() as u64);
        data.put_slice(&self.data);
    }

    fn len(&self) -> usize {
        Self::min_len()
            // data
            + self.data.len()
    }

    fn min_len() -> usize {
        // type
        std::mem::size_of::<u8>() +
            // id
            std::mem::size_of::<u16>()
            // offset
            + std::mem::size_of::<u64>()
            // length
            + std::mem::size_of::<u64>()
    }
}

impl Debug for StreamDataFrame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamDataFrame")
            .field("id", &self.id)
            .field("offset", &self.offset)
            .field("len", &self.data.len())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct StreamDataMeta {
    pub id: StreamId,
    pub range: Range<u64>,
}

#[derive(Clone, Debug)]
pub struct MaxStreamDataFrame {
    pub(crate) id: StreamId,
    pub(crate) max_data: u64,
}

impl MaxStreamDataFrame {
    pub fn meta(&self) -> MaxStreamDataMeta {
        MaxStreamDataMeta { id: self.id }
    }
}

impl Serializable for MaxStreamDataFrame {
    fn decode(data: &mut impl Buf) -> Self {
        let id = data.get_u16();
        let max_data = data.get_u64();

        Self { id, max_data }
    }

    fn encode(self, data: &mut impl BufMut) {
        data.put_u16(self.id);
        data.put_u64(self.max_data);
    }

    fn min_len() -> usize {
        // type
        std::mem::size_of::<u8>() +
            // id
            std::mem::size_of::<u16>()
            // max_data
            + std::mem::size_of::<u64>()
    }
}

#[derive(Clone, Debug)]
pub struct MaxStreamDataMeta {
    pub id: StreamId,
}
