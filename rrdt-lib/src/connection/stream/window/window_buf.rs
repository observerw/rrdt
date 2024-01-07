use crate::types::Chunk;
use bytes::BytesMut;
use std::ops::Range;
use tokio::io;

/// 带有偏移量的缓冲区
pub struct WindowBuf {
    buf: BytesMut,

    /// buf的起始偏移量
    start: u64,
}

impl WindowBuf {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: BytesMut::with_capacity(cap),
            start: 0,
        }
    }

    pub fn zeroed(len: usize) -> Self {
        Self {
            buf: BytesMut::zeroed(len),
            start: 0,
        }
    }

    /// 读取指定offset范围内的数据
    ///
    /// 返回的数据长度可能小于指定范围的长度
    pub fn read(&self, range: Range<u64>) -> Chunk {
        assert!(range.start >= self.start);

        let len = std::cmp::min((range.end - range.start) as usize, self.buf.len());

        let start = (range.start - self.start) as usize;
        let end = start + len;
        let buf = &self.buf[start..end];

        (buf.to_vec().into(), range.start)
    }

    pub fn write(&mut self, (data, offset): Chunk) -> io::Result<usize> {
        assert!(offset >= self.start);

        let start = (offset - self.start) as usize;
        let end = start + data.len();

        if end > self.buf.len() {
            self.buf.resize(end, 0);
        }

        self.buf[start..end].copy_from_slice(&data);

        Ok(data.len())
    }

    pub fn extend_from_slice(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn split_to(&mut self, offset: u64) -> BytesMut {
        assert!(offset >= self.start);

        let at = (offset - self.start) as usize;
        let buf = self.buf.split_to(at);

        self.start = offset;

        buf
    }

    pub fn resize(&mut self, new_len: usize, value: u8) {
        self.buf.resize(new_len, value);
    }

    pub fn start(&self) -> u64 {
        self.start
    }
}
