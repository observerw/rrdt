pub const K: usize = 1024;
pub const M: usize = 1024 * K;

/// 每个stream传输的数据量
pub const STREAM_CHUNK_SIZE: usize = 100 * M;

pub const CLIENT_ADDR: &str = "192.168.10.5:2333";
pub const SERVER_ADDR: &str = "192.168.10.3:2334";

pub const FILE_PATH: &str = "output.bin";
