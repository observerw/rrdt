use std::time::Duration;

/// 接收多少个顺序packet后发送一次ack
pub const DEFAULT_ACK_WAIT_COUNT: u64 = 2;

/// packet最大延迟发送时间
pub const MAX_PACKET_DELAY: Duration = Duration::from_millis(25);
