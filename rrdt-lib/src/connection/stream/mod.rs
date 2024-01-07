use self::{recv_stream::RecvStreamInner, send_stream::SendStreamInner};
use super::packetizer::Packetizer;
use crate::types::StreamId;
use actix::prelude::*;
use bytes::Buf;
use tokio::io;

pub mod recv_stream;
pub mod send_stream;
mod window;

#[derive(Clone)]
pub struct RecvStream {
    id: StreamId,
    inner: Addr<RecvStreamInner>,
}

impl RecvStream {
    pub(crate) fn new(id: StreamId, addrs: Addrs) -> Self {
        let inner = RecvStreamInner::new(id, addrs.clone()).start();

        Self { id, inner }
    }

    pub async fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let resp = self
            .inner
            .send(recv_stream::Read { len: buf.len() })
            .await
            .unwrap();

        // 二次等待，直到有数据可读
        if let Some(mut data) = resp.await.unwrap()? {
            let recv_len = data.len();
            data.copy_to_slice(&mut buf[..recv_len]);

            Ok(recv_len)
        } else {
            Ok(0)
        }
    }

    pub async fn close(self) {
        let closing = self.inner.send(recv_stream::Close).await.unwrap();
        let _ = closing.await.unwrap();
        eprintln!("recv stream {:?} closed", self.id);
    }

    pub fn id(&self) -> StreamId {
        self.id
    }

    pub(crate) fn inner(&self) -> &Addr<RecvStreamInner> {
        &self.inner
    }
}

#[derive(Clone)]
pub struct SendStream {
    id: StreamId,
    inner: Addr<SendStreamInner>,
}

impl SendStream {
    pub(crate) fn new(id: StreamId, addrs: Addrs) -> Self {
        let inner = SendStreamInner::new(id, addrs.clone()).start();

        Self { id, inner }
    }

    pub async fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner
            .send(send_stream::Write {
                data: buf.to_vec().into(),
            })
            .await
            .unwrap()
    }

    /// 声明所有数据已写入
    pub fn wrote(&self) {
        self.inner.do_send(send_stream::Wrote);
    }

    pub async fn close(self) {
        let closing = self.inner.send(send_stream::Close).await.unwrap();
        let _ = closing.await.unwrap();
        eprintln!("send stream {:?} closed", self.id);
    }

    pub fn id(&self) -> StreamId {
        self.id
    }

    pub(crate) fn inner(&self) -> &Addr<SendStreamInner> {
        &self.inner
    }
}

#[derive(Clone)]
pub struct Addrs {
    pub packetizer: Addr<Packetizer>,
}
