use crate::{connection::TransportParams, serializable::Serializable};

#[derive(Clone, Debug)]
pub struct HandshakeFrame {
    pub params: TransportParams,
}

impl Serializable for HandshakeFrame {
    fn decode(data: &mut impl bytes::Buf) -> Self {
        let params = TransportParams::decode(data);
        Self { params }
    }

    fn encode(self, data: &mut impl bytes::BufMut) {
        self.params.encode(data);
    }

    fn min_len() -> usize {
        // type
        std::mem::size_of::<u8>() +
            // params
            TransportParams::min_len()
    }
}
