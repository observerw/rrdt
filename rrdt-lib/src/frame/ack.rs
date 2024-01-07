use super::constant::DEFAULT_ACK_RANGES_LIMIT;
use crate::{
    serializable::Serializable,
    types::PacketNum,
    utils::{range_ext::RangeExt, range_set::RangeSet},
};
use bytes::{Buf, BufMut};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct AckFrame {
    /// 最大已确认包号
    pub largest_ack: PacketNum,
    pub delay: Duration,
    /// 包含最大已确认包号的第一个ack range的长度
    pub first_ack_range: u16,
    pub ack_ranges: Vec<AckRange>,
}

impl AckFrame {
    pub fn set_delay(&mut self, delay: Duration) {
        self.delay = delay;
    }

    /// 减小ack frame的大小，方法为按顺序删除最老的ack ranges
    pub fn reduce_to(&mut self, len: usize) {
        while !self.ack_ranges.is_empty() && self.len() > len {
            self.ack_ranges.pop();
        }
    }
}

impl Serializable for AckFrame {
    fn decode(data: &mut impl Buf) -> Self {
        let largest_ack = data.get_u64();
        let delay = Duration::from_millis(data.get_u64());
        let ack_range_count = data.get_u16();
        let first_ack_range = data.get_u16();
        let mut ack_ranges = Vec::with_capacity(ack_range_count as usize);
        for _ in 0..ack_range_count {
            let ack_range = AckRange::decode(data);
            ack_ranges.push(ack_range);
        }

        Self {
            largest_ack,
            delay,
            first_ack_range,
            ack_ranges,
        }
    }

    fn encode(self, data: &mut impl BufMut) {
        data.put_u64(self.largest_ack);
        data.put_u64(self.delay.as_millis() as u64);
        data.put_u16(self.ack_ranges.len() as u16);
        data.put_u16(self.first_ack_range);
        for ack_range in self.ack_ranges {
            ack_range.encode(data);
        }
    }

    fn len(&self) -> usize {
        Self::min_len()
            // ack_ranges
            + self
                .ack_ranges
                .iter()
                .map(|ack_range| ack_range.len())
                .sum::<usize>()
    }

    fn min_len() -> usize {
        // type
        std::mem::size_of::<u8>() +
            // largest_ack
            std::mem::size_of::<u64>()
            // delay
            + std::mem::size_of::<u64>()
            // ack_range_count
            + std::mem::size_of::<u16>()
            // first_ack_range
            + std::mem::size_of::<u16>()
    }
}

#[derive(Debug, Clone)]
pub struct AckRange {
    gap: u16,
    length: u16,
}

impl Serializable for AckRange {
    fn decode(data: &mut impl Buf) -> Self {
        let gap = data.get_u16();
        let length = data.get_u16();
        Self { gap, length }
    }

    fn encode(self, data: &mut impl BufMut) {
        data.put_u16(self.gap);
        data.put_u16(self.length);
    }

    fn min_len() -> usize {
        // gap
        std::mem::size_of::<u16>()
            // length
            + std::mem::size_of::<u16>()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AckSpans {
    set: RangeSet,
    limit: usize,
}

impl AckSpans {
    pub fn new() -> Self {
        Self {
            set: RangeSet::new(),
            limit: DEFAULT_ACK_RANGES_LIMIT,
        }
    }

    pub fn with_ranges_limit(self, limit: usize) -> Self {
        Self { limit, ..self }
    }

    pub fn insert(&mut self, x: u64) -> bool {
        self.set.insert_one(x)
    }

    pub fn contains(&self, x: u64) -> bool {
        self.set.contains(x)
    }
}

impl From<AckFrame> for AckSpans {
    fn from(
        AckFrame {
            largest_ack,
            first_ack_range,
            ack_ranges,
            ..
        }: AckFrame,
    ) -> Self {
        let mut set = RangeSet::new();

        let end = largest_ack + 1;
        let start = end - first_ack_range as PacketNum;
        let first_range = start..end;

        set.insert(first_range);

        for AckRange { gap, length } in ack_ranges {
            // TODO
            let current = set.first().unwrap();
            let end = current.start - gap as PacketNum;
            let start = end - length as PacketNum;
            set.insert(start..end);
        }

        Self {
            set,
            limit: DEFAULT_ACK_RANGES_LIMIT,
        }
    }
}

impl Into<AckFrame> for AckSpans {
    fn into(mut self) -> AckFrame {
        if self.set.is_empty() {
            return AckFrame::default();
        }

        let mut ack_ranges = Vec::with_capacity(self.set.len());

        let first = self.set.pop_back().unwrap();
        let largest_ack = first.end - 1;
        let first_ack_range = (first.end - first.start) as u16;

        let mut current = first;
        for range in self.set.iter().rev() {
            let gap = (current.start - range.end) as u16;
            let length = range.len() as u16;
            ack_ranges.push(AckRange { gap, length });
            if ack_ranges.len() >= self.limit {
                break;
            }
            current = range;
        }

        AckFrame {
            largest_ack,
            first_ack_range,
            ack_ranges,
            ..AckFrame::default()
        }
    }
}

#[test]
fn test() {
    let mut spans = AckSpans::new();

    spans.insert(0);
    spans.insert(1);
    spans.insert(2);
    spans.insert(4);
    spans.insert(6);
    spans.insert(9);

    eprintln!("spans = {:?}", spans);
    let frame: AckFrame = spans.clone().into();
    eprintln!("frame = {:?}", frame);
    let recv_spans: AckSpans = frame.into();
    eprintln!("recv_spans = {:?}", recv_spans);

    assert_eq!(spans, recv_spans);
}
