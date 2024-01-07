use std::time::Duration;

pub const HANDSHAKE_TYPE: u8 = 0x01;
pub const STREAM_TYPE: u8 = 0x02;
pub const STREAM_FIN_TYPE: u8 = 0x03;
pub const ACK_TYPE: u8 = 0x04;
pub const MAX_STREAM_DATA_TYPE: u8 = 0x05;

pub const DEFAULT_ACK_RANGES_LIMIT: usize = 200;
