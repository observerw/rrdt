use super::window::{Chunk, SendWindow};
use crate::{
    frame::stream::StreamDataFrame,
    serializable::Serializable,
    types::{Requester, Responder, StreamId},
};
use actix::prelude::*;
use bytes::Bytes;
use std::ops::Range;
use tokio::{io, sync::oneshot};

pub struct SendStreamInner {
    id: StreamId,
    addrs: super::Addrs,

    window: SendWindow,

    state: State,

    /// 应用层声明所有数据已写入
    wrote: bool,

    /// 应用层已调用`close`，等待stream关闭
    closing: Option<Responder<()>>,
}

impl SendStreamInner {
    pub fn new(id: StreamId, addrs: super::Addrs) -> Self {
        Self {
            id,
            addrs,
            window: SendWindow::new(),
            state: State::Ready,
            wrote: false,
            closing: None,
        }
    }

    fn close(&mut self) {
        if let Some(closing) = self.closing.take() {
            let _ = closing.send(());
        }
    }
}

impl Actor for SendStreamInner {
    type Context = Context<Self>;
}

impl Handler<Read> for SendStreamInner {
    type Result = io::Result<Option<StreamDataFrame>>;

    fn handle(&mut self, Read { bytes }: Read, _ctx: &mut Self::Context) -> Self::Result {
        let data_len = bytes - StreamDataFrame::min_len();

        match self.state {
            State::Ready | State::Send => {
                if let Some((Chunk(data, offset), fin)) = self.window.read(data_len)? {
                    if fin {
                        // 如果发送了fin frame则进入`DataSent`状态
                        self.state = State::DataSent;
                    } else {
                        self.state = State::Send;
                    }

                    Ok(Some(StreamDataFrame {
                        id: self.id,
                        offset,
                        data,
                        fin,
                    }))
                } else {
                    Ok(None)
                }
            }
            State::DataSent => {
                if let Some((Chunk(data, offset), fin)) = self.window.read_retransmit(data_len)? {
                    Ok(Some(StreamDataFrame {
                        id: self.id,
                        offset,
                        data,
                        fin,
                    }))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
}

impl Handler<Write> for SendStreamInner {
    type Result = io::Result<usize>;

    fn handle(&mut self, Write { data }: Write, _ctx: &mut Self::Context) -> Self::Result {
        // 进入关闭流程后，不再接受新的写入
        if self.wrote {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "stream has been reset",
            ));
        }

        self.window.write(data)
    }
}

impl Handler<MaxData> for SendStreamInner {
    type Result = ();

    fn handle(&mut self, MaxData(max_data): MaxData, _ctx: &mut Self::Context) -> Self::Result {
        self.window.set_max_data(max_data)
    }
}

impl Handler<Ack> for SendStreamInner {
    type Result = ();

    fn handle(&mut self, Ack(range): Ack, _ctx: &mut Self::Context) -> Self::Result {
        self.window.ack(range);

        match self.state {
            State::DataSent if self.window.done() => {
                self.state = State::DataRecvd;
                self.close();
            }
            _ => {}
        }
    }
}

impl Handler<Retransmit> for SendStreamInner {
    type Result = ();

    fn handle(&mut self, Retransmit(range): Retransmit, _ctx: &mut Self::Context) -> Self::Result {
        self.window.retransmit(range);
    }
}

impl Handler<Available> for SendStreamInner {
    type Result = usize;

    fn handle(&mut self, _: Available, _ctx: &mut Self::Context) -> Self::Result {
        self.window.available()
    }
}

impl Handler<Wrote> for SendStreamInner {
    type Result = ();

    fn handle(&mut self, _: Wrote, _ctx: &mut Self::Context) -> Self::Result {
        self.window.set_wrote();
    }
}

impl Handler<Close> for SendStreamInner {
    type Result = Response<Requester<()>>;

    fn handle(&mut self, _: Close, _ctx: &mut Self::Context) -> Self::Result {
        let (resp, req) = oneshot::channel();

        match self.state {
            State::DataRecvd | State::ResetRecvd => {
                let _ = resp.send(());
            }
            _ => {
                self.closing = Some(resp);
            }
        }

        Response::reply(req)
    }
}

/// 从窗口中读取数据，将读取到的数据作为一个`StreamDataFrame`返回
///
/// `StreamDataFrame`可能超过Frame的最大长度，会在`Packetizer`中进行分片
#[derive(Message)]
#[rtype(result = "io::Result<Option<StreamDataFrame>>")]
pub struct Read {
    pub bytes: usize,
}

#[derive(Message)]
#[rtype(result = "io::Result<usize>")]
pub struct Write {
    pub data: Bytes,
}

/// 设置`max_stream_data`
#[derive(Message)]
#[rtype(result = "()")]
pub struct MaxData(pub u64);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Ack(pub Range<u64>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Retransmit(pub Range<u64>);

#[derive(Message)]
#[rtype(result = "usize")]
pub struct Available;

/// 告知stream所有数据已写入
#[derive(Message)]
#[rtype(result = "()")]
pub struct Wrote;

/// 等待stream关闭
#[derive(Message)]
#[rtype(result = "Requester<()>")]
pub struct Close;

#[derive(Debug)]
enum State {
    Ready,
    Send,
    DataSent,
    DataRecvd,

    ResetSent,
    ResetRecvd,
}
