use bytes::{Buf, BufMut};

pub trait Serializable {
    /// 从data中解码数据，并移动data的读指针
    fn decode(data: &mut impl Buf) -> Self;

    /// 将数据编码到data中，并移动data的写指针
    fn encode(self, data: &mut impl BufMut);

    /// 编码后的数据最小长度
    ///
    /// 对于ack、stream之类可能包含变长字段的数据结构，最小长度即为不包含变长字段的长度
    ///
    /// 对于不包含变长字段的数据结构，该长度即为数据结构的长度
    fn min_len() -> usize;

    /// 编码后的数据长度，包含变长字段
    fn len(&self) -> usize {
        Self::min_len()
    }
}
