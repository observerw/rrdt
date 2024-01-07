use super::streams::{self, StreamsInner};
use super::{ack_sender, ConnectionContext};
use super::{ack_sender::AckSender, inflight::Inflight};
use crate::connection::inflight;
use crate::frame::StreamFrame;
use crate::packet::MAX_PACKET_SIZE;
use crate::serializable::Serializable;
use crate::{frame::Frame, packet::Packet};
use actix::prelude::*;
use tokio::io;
use tokio::time::Instant;

pub struct Receiver {
    ctx: ConnectionContext,
    addrs: Addrs,
}

impl Receiver {
    pub fn new(ctx: ConnectionContext, addrs: Addrs) -> Self {
        Self { ctx, addrs }
    }
}

impl Actor for Receiver {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let receiver = ctx.address();
        let socket = self.ctx.socket.clone();
        // 将socket接收数据的循环部署到独立的线程中
        ctx.spawn(
            async move {
                let mut packet_buf = [0u8; MAX_PACKET_SIZE];
                loop {
                    let n = socket.recv(&mut packet_buf).await;
                    let packet = n.map(|n| Packet::decode(&mut &packet_buf[..n]));
                    receiver.do_send(Recv(packet));
                }
            }
            .into_actor(self),
        );
    }
}

impl Handler<Recv> for Receiver {
    type Result = ();

    fn handle(&mut self, Recv(packet): Recv, ctx: &mut Self::Context) -> Self::Result {
        if let Ok(packet) = packet {
            let packet_num = packet.packet_num();
            let is_ack_eliciting = packet.is_ack_eliciting();
            let instant = Instant::now();

            let addrs = self.addrs.clone();
            ctx.spawn(
                async move {
                    for frame in packet.into_frames() {
                        match frame {
                            // 收到ack frame时，更新inflight信息
                            Frame::Ack(frame) => {
                                addrs
                                    .inflight
                                    .send(inflight::Ack { frame, instant })
                                    .await
                                    .unwrap();
                            }
                            // 收到stream frame时，将其分发给对应的stream
                            Frame::Stream(frame) => {
                                addrs
                                    .streams
                                    .send(streams::Dispatch(StreamFrame::Data(frame)))
                                    .await
                                    .unwrap();
                            }
                            Frame::MaxStreamData(frame) => {
                                addrs
                                    .streams
                                    .send(streams::Dispatch(StreamFrame::MaxData(frame)))
                                    .await
                                    .unwrap();
                            }
                            Frame::Handshake(_) => {}
                        }
                    }
                }
                .into_actor(self),
            );

            // 必须在所有frame处理完毕之后再进行ack
            self.addrs.ack_sender.do_send(ack_sender::Recv {
                packet_num,
                is_ack_eliciting,
                instant,
            });
        } else {
            // TODO actor怎么做错误处理？
            ctx.stop();
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Recv(pub io::Result<Packet>);

#[derive(Clone)]
pub struct Addrs {
    pub inflight: Addr<Inflight>,
    pub ack_sender: Addr<AckSender>,
    pub streams: Addr<StreamsInner>,
}
