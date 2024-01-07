use super::{
    bcast::{AckedBcast, LostBcast},
    inflight::{self, Inflight},
    ConnectionContext,
};
use crate::{
    packet::{Packet, PacketMeta, MAX_PACKET_SIZE},
    serializable::Serializable,
};
use actix::prelude::*;
use bytes::BytesMut;
use tokio::time::Instant;

pub struct Sender {
    ctx: ConnectionContext,
    addrs: Addrs,

    packet_buf: BytesMut,
}

impl Sender {
    pub fn new(ctx: ConnectionContext, addrs: Addrs) -> Self {
        Self {
            ctx,
            addrs,
            packet_buf: BytesMut::with_capacity(MAX_PACKET_SIZE),
        }
    }
}

impl Actor for Sender {
    type Context = Context<Self>;
}

impl Handler<SendPacket> for Sender {
    type Result = ();

    fn handle(&mut self, SendPacket(packet): SendPacket, ctx: &mut Self::Context) -> Self::Result {
        let size = packet.len();
        let socket = self.ctx.socket.clone();
        let mut buf = self.packet_buf.clone();
        let inflight = self.addrs.inflight.clone();

        ctx.spawn(
            async move {
                let meta = packet.meta(Instant::now());
                packet.encode(&mut buf);
                let _ = socket.send(&buf[..size]).await;
                inflight.do_send(inflight::Sent(meta));
            }
            .into_actor(self),
        );
    }
}

impl Handler<AckedBcast> for Sender {
    type Result = ();

    fn handle(&mut self, AckedBcast(meta): AckedBcast, _ctx: &mut Self::Context) -> Self::Result {
        for PacketMeta { sent, bytes, .. } in meta {
            self.ctx.congestion.write().unwrap().on_ack(sent, bytes);
        }
    }
}

impl Handler<LostBcast> for Sender {
    type Result = ();

    fn handle(
        &mut self,
        LostBcast(PacketMeta { sent, bytes, .. }): LostBcast,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let now = Instant::now();
        self.ctx
            .congestion
            .write()
            .unwrap()
            .on_loss(now, sent, bytes);
    }
}

/// 发送若干frame
#[derive(Message)]
#[rtype(result = "()")]
pub struct SendPacket(pub Packet);

pub struct Addrs {
    pub inflight: Addr<Inflight>,
    // pub congestion: Addr<Congestion>,
}
