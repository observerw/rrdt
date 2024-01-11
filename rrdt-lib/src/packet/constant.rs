use crate::constant::K;

pub const MAX_PACKET_SIZE: usize = 8 * K;

pub const HANDSHAKE_PACKET_TYPE: u8 = 0x01;
pub const HANDSHAKE_DONE_PACKET_TYPE: u8 = 0x02;
pub const COMPRESSED_PACKET_TYPE: u8 = 0x03;
