use super::{
    constant::MAX_PACKET_DELAY,
    sender::{self, Sender},
    ConnectionContext,
};
use crate::{
    frame::{stream::StreamDataFrame, Frame},
    packet::Packet,
    serializable::Serializable,
    types::PacketNum,
};
use actix::prelude::*;
use std::cell::RefCell;

pub struct Packetizer {
    ctx: ConnectionContext,
    addrs: Addrs,
    packet_num: PacketNum,
    current: RefCell<Packet>,

    timer_handle: Option<SpawnHandle>,
}

impl Packetizer {
    pub fn new(ctx: ConnectionContext, addrs: Addrs) -> Self {
        Self {
            ctx,
            addrs,
            current: RefCell::new(Packet::new(0)),
            // 下一个packet的编号，初始已经有了0号packet所以从1开始
            packet_num: 1,
            timer_handle: None,
        }
    }

    fn remaining(&self) -> usize {
        self.current.borrow().remaining()
    }

    fn is_empty(&self) -> bool {
        self.current.borrow().is_empty()
    }

    fn push(&self, frame: Frame) {
        self.current.borrow_mut().push(frame);
    }

    fn insert(&mut self, ctx: &mut Context<Self>, frame: Frame) {
        // frame过大时先把当前packet发送出去，将frame放在下一个packet中
        if frame.len() > self.remaining() {
            self.send(ctx);
        }

        // 如果是在一个新的packet中添加frame则部署超时任务
        if self.is_empty() {
            ctx.notify_later(Timeout, MAX_PACKET_DELAY);
        }

        self.push(frame);

        // packet已满，立即发送
        // TODO
        if self.remaining() < 20 {
            self.send(ctx);
        }
    }

    /// 立即将当前packet发送出去
    fn send(&mut self, ctx: &mut Context<Self>) {
        if self.is_empty() {
            return;
        }

        let next = Packet::new(self.packet_num);
        self.packet_num += 1;

        let prev = self.current.replace(next);
        self.addrs.sender.do_send(sender::SendPacket(prev));

        if let Some(handle) = self.timer_handle {
            ctx.cancel_future(handle);
        }
    }
}

impl Actor for Packetizer {
    type Context = Context<Self>;
}

impl Handler<Send> for Packetizer {
    type Result = ();

    fn handle(&mut self, Send(frame): Send, ctx: &mut Self::Context) -> Self::Result {
        match frame {
            Frame::Stream(mut frame) => {
                // stream frame过大时可以进行拆分
                let min_len = StreamDataFrame::min_len();
                loop {
                    let len = frame.len();
                    let remaining = self.remaining();

                    if ((min_len + 1)..len).contains(&remaining) {
                        let splitted = frame.split_to(remaining);

                        self.insert(ctx, Frame::Stream(splitted));
                    } else {
                        break;
                    }
                }

                self.insert(ctx, Frame::Stream(frame));
            }
            Frame::MaxStreamData(frame) => {
                self.insert(ctx, Frame::MaxStreamData(frame));
            }
            Frame::Ack(frame) => {
                self.insert(ctx, Frame::Ack(frame));
                // 包含ACK frame的packet应该立即发送
                self.send(ctx);
            }
            _ => {}
        }
    }
}

impl Handler<Timeout> for Packetizer {
    type Result = ();

    fn handle(&mut self, _: Timeout, ctx: &mut Self::Context) -> Self::Result {
        self.send(ctx);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Send(pub Frame);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Timeout;

pub struct Addrs {
    pub sender: Addr<Sender>,
}

#[actix_rt::test]
async fn test() {}
