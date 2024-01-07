use super::{constant::MAX_WINDOW_SIZE, window_buf::WindowBuf, Chunk};
use crate::utils::{range_ext::RangeExt, range_set::RangeSet};
use bytes::Bytes;
use std::{io, ops::Range};

pub struct SendWindow {
    buf: WindowBuf,

    /// 能发送的最大偏移量
    max_data: u64,

    /// 已发送过的数据的右边界偏移量，或未发送数据的左边界偏移量
    sent_offset: u64,

    /// 已写入过数据的右边界偏移量，或未写入数据的左边界偏移量
    ///
    /// 有可能大于 `max_data`
    wrote_offset: u64,

    /// 已经确认的数据段
    acks: RangeSet,

    /// 已发送但丢失的数据段
    retransmits: RangeSet,

    /// 应用层声明所有数据已写入
    wrote: bool,
}

impl SendWindow {
    pub fn new() -> Self {
        Self {
            max_data: MAX_WINDOW_SIZE as u64,
            // send window的新数据会被extend到buf的末尾，因此这里使用with_capacity即可
            buf: WindowBuf::with_capacity(MAX_WINDOW_SIZE),
            sent_offset: 0,
            wrote_offset: 0,
            acks: RangeSet::new(),
            retransmits: RangeSet::new(),
            wrote: false,
        }
    }

    fn fin(&self, Chunk(data, offset): &Chunk) -> bool {
        self.wrote && offset + data.len() as u64 == self.wrote_offset
    }

    pub fn write(&mut self, data: Bytes) -> io::Result<usize> {
        if self.wrote {
            return Ok(0);
        }

        self.buf.extend_from_slice(&data);
        self.wrote_offset += data.len() as u64;
        Ok(data.len())
    }

    pub fn read_retransmit(&mut self, len: usize) -> io::Result<Option<(Chunk, bool)>> {
        if let Some(mut range) = self.retransmits.pop_front() {
            let read_len = std::cmp::min(len, range.len());

            let read_range = range.split_to(read_len as u64);

            // 若读取的数据长度小于数据段长度，则将剩余数据段重新加入重传队列
            if range.len() > 0 {
                self.retransmits.insert(range);
            }

            let chunk = self.buf.read(read_range);
            let fin = self.fin(&chunk);
            Ok(Some((chunk, fin)))
        } else {
            Ok(None)
        }
    }

    pub fn read(&mut self, len: usize) -> io::Result<Option<(Chunk, bool)>> {
        if let Some(chunk) = self.read_retransmit(len)? {
            Ok(Some(chunk))
        } else if self.available() == 0 {
            return Ok(None);
        } else {
            let chunk_len = std::cmp::min(len, self.available());
            let chunk = self
                .buf
                .read(self.sent_offset..self.sent_offset + chunk_len as u64);

            self.sent_offset += chunk_len as u64;

            let fin = self.fin(&chunk);
            Ok(Some((chunk, fin)))
        }
    }

    pub fn set_max_data(&mut self, max_data: u64) {
        if max_data <= self.max_data {
            return;
        }

        self.max_data = max_data;
    }

    pub fn ack(&mut self, range: Range<u64>) {
        if range.is_empty() || range.start < self.acked() {
            return;
        }

        // 若已经认为丢包，但随后又收到了ack，则直接无视ack
        if self.retransmits.contains_range(&range) {
            return;
        }

        self.acks.insert(range);

        let min_ack = self.acks.min().unwrap();
        // 若有从consumed开始的连续ACK段，则将对应数据从缓冲区中移除
        if min_ack == self.buf.start() {
            let range = self.acks.pop_front().unwrap();
            let _ = self.buf.split_to(range.end);
        }
    }

    /// 标记某数据段丢失，需要重传
    pub fn retransmit(&mut self, range: Range<u64>) {
        if range.is_empty() || range.start < self.acked() {
            return;
        }

        self.retransmits.insert(range);
    }

    /// 当前可发送的数据长度
    ///
    /// 定义为已发数据到已写数据或 `max_data` 之间的长度较小值
    pub fn available(&self) -> usize {
        let upper = std::cmp::min(self.max_data, self.wrote_offset);
        (upper - self.sent_offset) as usize
    }

    /// 当前已发送且已确认过的偏移量
    pub fn acked(&self) -> u64 {
        self.buf.start()
    }

    /// 所有数据都发送完且已确认
    pub fn done(&self) -> bool {
        self.acked() == self.wrote_offset
    }

    pub fn set_wrote(&mut self) {
        self.wrote = true;
    }
}
