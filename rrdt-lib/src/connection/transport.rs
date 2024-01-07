use crate::serializable::Serializable;
use std::time::Duration;

use super::constant::DEFAULT_MAX_ACK_DELAY;

/// 连接建立过程中双方声明的一些传输参数
///
/// 连接双方可以各自独立的声明自己想要的参数
#[derive(Clone, Debug)]
pub struct TransportParams {
    /// 发送方承诺发送ack的最大延迟时间，单位毫秒
    pub max_ack_delay: Duration,

    /// initial value for the maximum amount of data that can be sent on the connection
    // pub initial_max_data: u64,

    /// 新建的stream的流量控制窗口的初始大小
    pub initial_max_stream_data: u64,

    /// 对端可以建立的最大双向stream数量
    // pub initial_max_streams: u16,

    /// 对端承诺会开启的stream数量
    ///
    /// 真正的QUIC中没有这个参数，这里只是为了简化逻辑
    pub streams: u16,
}

impl TransportParams {
    pub fn with_max_ack_delay(mut self, max_ack_delay: Duration) -> Self {
        self.max_ack_delay = max_ack_delay;
        self
    }

    pub fn with_initial_max_stream_data(mut self, initial_max_stream_data: u64) -> Self {
        self.initial_max_stream_data = initial_max_stream_data;
        self
    }

    pub fn with_streams(mut self, streams: u16) -> Self {
        self.streams = streams;
        self
    }
}

impl Default for TransportParams {
    fn default() -> Self {
        Self {
            max_ack_delay: DEFAULT_MAX_ACK_DELAY,
            // initial_max_data: 1024 * 1024,
            initial_max_stream_data: 1024 * 1024,
            // initial_max_streams: 10,
            streams: 10,
        }
    }
}

impl Serializable for TransportParams {
    fn decode(data: &mut impl bytes::Buf) -> Self {
        let max_ack_delay = data.get_u64();
        let initial_max_stream_data = data.get_u64();
        let streams = data.get_u16();

        Self {
            max_ack_delay: Duration::from_millis(max_ack_delay),
            initial_max_stream_data,
            streams,
        }
    }

    fn encode(self, data: &mut impl bytes::BufMut) {
        data.put_u64(self.max_ack_delay.as_millis() as u64);
        data.put_u64(self.initial_max_stream_data);
        data.put_u16(self.streams);
    }

    fn min_len() -> usize {
        // max_ack_delay
        std::mem::size_of::<u64>() +
            // initial_max_stream_data
            std::mem::size_of::<u64>() +
            // initial_max_streams
            std::mem::size_of::<u16>()
    }
}
