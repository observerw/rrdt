use bytes::Bytes;

use crate::types::Offset;

mod constant;
mod recv_window;
mod send_window;
mod window_buf;

pub use {recv_window::RecvWindow, send_window::SendWindow};

pub struct Chunk(pub Bytes, pub Offset);
