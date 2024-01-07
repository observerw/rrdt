use super::{constant::MAX_WINDOW_SIZE, window_buf::WindowBuf, Chunk};
use crate::utils::{range_ext::RangeExt, range_set::RangeSet};
use bytes::Bytes;
use tokio::io;

pub struct RecvWindow {
    buf: WindowBuf,

    pub recv: RangeSet,

    /// 窗口左边界偏移量
    start: u64,

    fin_offset: Option<u64>,
}

impl RecvWindow {
    pub fn new() -> Self {
        Self {
            // recv window必须要保证窗口内的数据全部已经初始化，因此这里使用zeroed
            buf: WindowBuf::zeroed(MAX_WINDOW_SIZE),
            recv: RangeSet::new(),
            start: 0,
            fin_offset: None,
        }
    }

    pub fn write(&mut self, Chunk(data, offset): Chunk, fin: bool) -> io::Result<usize> {
        if data.len() == 0 || offset < self.consumed() {
            return Ok(0);
        }

        let right_offset = offset + data.len() as u64;
        let range = offset..right_offset;

        let n = self.buf.write(Chunk(data, offset))?;

        self.recv.insert(range);

        if fin {
            self.fin_offset = Some(right_offset);
        }

        Ok(n)
    }

    pub fn read(&mut self, len: usize) -> io::Result<Option<Bytes>> {
        if self.recv.is_empty() {
            Ok(None)
        } else {
            let min = self.recv.min().unwrap();
            if min != self.consumed() {
                return Ok(None);
            }

            let mut range = self.recv.pop_front().unwrap();
            let read_len = std::cmp::min(len, (range.end - range.start) as usize);
            let read_offset = range.start + read_len as u64;

            let _ = range.split_to(read_len as u64);
            if range.len() > 0 {
                self.recv.insert(range);
            }

            let consume_buf = self.buf.split_to(read_offset).freeze();

            Ok(Some(consume_buf))
        }
    }

    /// 启发式判断是否需要更新窗口左边界，判断依据为窗口已消费数据量是否超过窗口大小的一半
    pub fn should_update(&self) -> bool {
        self.consumed() > self.start + MAX_WINDOW_SIZE as u64 / 2
    }

    /// 无条件更新窗口左边界，返回新的 `max_stream_data`
    pub fn update(&mut self) -> u64 {
        self.start = self.consumed();
        self.buf.resize(MAX_WINDOW_SIZE, 0);

        self.max_stream_data()
    }

    pub fn max_stream_data(&self) -> u64 {
        self.start + MAX_WINDOW_SIZE as u64
    }

    pub fn recvd(&self) -> bool {
        self.fin_offset.is_some_and(|offset| {
            let range = self.consumed()..offset;
            self.recv.len() == 1 && self.recv.first().unwrap() == range
        })
    }

    /// 窗口里没有数据了
    pub fn done(&self) -> bool {
        self.fin_offset
            .is_some_and(|offset| offset == self.consumed())
    }

    /// 目前已经消费的数据的右边界偏移量
    pub fn consumed(&self) -> u64 {
        self.buf.start()
    }
}
