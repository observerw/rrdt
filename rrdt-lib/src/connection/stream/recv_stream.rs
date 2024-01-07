use super::window::RecvWindow;
use crate::{
    connection::{packetizer, sender::Sender},
    frame::{stream::MaxStreamDataFrame, Frame},
    types::{Requester, Responder, StreamId},
};
use actix::prelude::*;
use bytes::Bytes;
use std::collections::VecDeque;
use tokio::{io, sync::oneshot};

pub struct RecvStreamInner {
    id: StreamId,
    addrs: super::Addrs,

    window: RecvWindow,

    /// 由于当前没有可读数据而等待中的读请求
    ///
    /// 在每次写入新数据后都会尝试处理这些请求
    pending: VecDeque<ReadRequest>,

    state: State,
    closing: Option<Responder<()>>,
}

impl RecvStreamInner {
    pub fn new(id: StreamId, addrs: super::Addrs) -> Self {
        Self {
            id,
            addrs,
            window: RecvWindow::new(),
            pending: VecDeque::new(),
            state: State::Recv,
            closing: None,
        }
    }

    fn close(&mut self) {
        if let Some(resp) = self.closing.take() {
            let _ = resp.send(());
        }
    }

    /// 处理读请求，返回该请求是否已经被处理（读取到数据或发生错误）
    ///
    /// 没有被成功处理的请求会重新被放入 `pending` 中
    fn handle_read_request(&mut self, req: ReadRequest) -> bool {
        match self.window.read(req.len) {
            Ok(None) => {
                self.pending.push_back(req);
                false
            }
            result @ _ => {
                let _ = req.resp.send(result);
                true
            }
        }
    }

    fn handle_pending(&mut self) {
        match self.state {
            // stream已经关闭，直接返回`Ok(None)`
            State::DataRead | State::ResetRead => {
                while let Some(req) = self.pending.pop_back() {
                    let _ = req.resp.send(Ok(None));
                }
            }
            State::Recv | State::SizeKnown | State::DataRecvd => {
                while let Some(req) = self.pending.pop_back() {
                    if !self.handle_read_request(req) {
                        break;
                    }
                }

                if matches!(self.state, State::DataRecvd) && self.window.done() {
                    // 转移到最终状态时需尝试发送关闭通知
                    self.state = State::DataRead;
                    self.close();
                }
            }
            _ => todo!(),
        }
    }
}

impl Actor for RecvStreamInner {
    type Context = Context<Self>;
}

impl Handler<Read> for RecvStreamInner {
    type Result = Response<ReadResp>;

    fn handle(&mut self, Read { len }: Read, ctx: &mut Self::Context) -> Self::Result {
        let (resp, req) = oneshot::channel();

        self.pending.push_front(ReadRequest { len, resp });

        self.handle_pending();

        if self.window.should_update() {
            ctx.notify(Update);
        }

        Response::reply(req)
    }
}

impl Handler<Write> for RecvStreamInner {
    type Result = io::Result<usize>;

    fn handle(
        &mut self,
        Write { data, offset, fin }: Write,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if matches!(self.state, State::ResetRecvd | State::ResetRead) {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "stream has been reset",
            ));
        }
        if matches!(self.state, State::DataRecvd | State::DataRead) {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "stream has been closed",
            ));
        }

        let result = self.window.write((data, offset), fin);

        if result.is_ok() {
            match self.state {
                // 接收到带fin的`StreamDataFrame`后进入`SizeKnown`状态
                State::Recv if fin => {
                    if self.window.recvd() {
                        self.state = State::DataRecvd;
                    } else {
                        self.state = State::SizeKnown;
                    }
                }
                // 进入到`SizeKnown`状态且已经收到了所有重传数据后进入`DataRecvd`状态
                State::SizeKnown if self.window.recvd() => {
                    self.state = State::DataRecvd;
                }
                _ => {}
            }

            // 写入新数据后，有可能能够处理等待中的读请求
            self.handle_pending();

            if self.window.should_update() {
                ctx.notify(Update);
            }
        }

        result
    }
}

impl Handler<Update> for RecvStreamInner {
    type Result = u64;

    fn handle(&mut self, _: Update, _ctx: &mut Self::Context) -> Self::Result {
        let max_data = self.window.update();

        match self.state {
            // 只有在`Recv`状态才有必要向对端发送 `max_stream_data` frame
            State::Recv => {
                self.addrs
                    .packetizer
                    .do_send(packetizer::Send(Frame::MaxStreamData(MaxStreamDataFrame {
                        id: self.id,
                        max_data,
                    })));
            }
            _ => {}
        }

        max_data
    }
}

impl Handler<Close> for RecvStreamInner {
    type Result = Response<Requester<()>>;

    fn handle(&mut self, _: Close, _ctx: &mut Self::Context) -> Self::Result {
        let (resp, req) = oneshot::channel();

        match self.state {
            // 已经处于关闭状态，直接返回
            State::DataRead | State::ResetRead => {
                let _ = resp.send(());
            }
            _ => {
                self.closing = Some(resp);
            }
        }

        Response::reply(req)
    }
}

/// 读取指定长度的数据
///
/// 注意：此处返回的是一个 `oneshot::Receiver`：
///   - 当stream没有处于关闭状态，且当窗口中目前没有足够的数据时，返回的 `Receiver` 会被挂起，直到有足够的数据时才会被唤醒；
///   - 当stream已经关闭时将会立即返回`Ok(None)`；
#[derive(Message)]
#[rtype(result = "ReadResp")]
pub struct Read {
    pub len: usize,
}

struct ReadRequest {
    pub len: usize,
    pub resp: Responder<io::Result<Option<Bytes>>>,
}

type ReadResp = Requester<io::Result<Option<Bytes>>>;

/// 写入数据
#[derive(Message)]
#[rtype(result = "io::Result<usize>")]
pub struct Write {
    pub data: Bytes,
    pub offset: u64,
    pub fin: bool,
}

/// 更新窗口并向对端发送 `max_stream_data` frame
#[derive(Message)]
#[rtype(result = "u64")]
pub struct Update;

#[derive(Message)]
#[rtype(result = "Requester<()>")]
pub struct Close;

pub struct Addrs {
    pub sender: Addr<Sender>,
}

#[derive(Debug)]
enum State {
    Recv,
    SizeKnown,
    DataRecvd,
    DataRead,
    ResetRecvd,
    ResetRead,
}
