use super::{
    bcast::{AckedBcast, ListenAckedBcast, ListenLostBcast, LostBcast},
    ConnectionContext,
};
use crate::{
    frame::ack::{AckFrame, AckSpans},
    packet::PacketMeta,
    types::PacketNum,
    utils::task_guard::TaskGuard,
};
use actix::prelude::*;
use std::collections::HashMap;
use tokio::time::Instant;

pub struct Inflight {
    ctx: ConnectionContext,

    acked_listeners: Vec<Recipient<AckedBcast>>,
    lost_listeners: Vec<Recipient<LostBcast>>,

    /// 当前正在传输的packet
    ///
    /// 记录该packet的meta信息，以及超时任务的guard
    packets: HashMap<PacketNum, (PacketMeta, TaskGuard)>,
}

impl Actor for Inflight {
    type Context = Context<Self>;
}

impl Inflight {
    pub fn new(ctx: ConnectionContext) -> Self {
        Self {
            ctx,
            acked_listeners: vec![],
            lost_listeners: vec![],
            packets: HashMap::with_capacity(2048),
        }
    }
}

impl Handler<Sent> for Inflight {
    type Result = ();

    /// 有新packet发出时将其注册到inflight中，并部署一个超时任务
    fn handle(&mut self, Sent(meta): Sent, _ctx: &mut Self::Context) -> Self::Result {
        // 如果该packet不是ack eliciting的，则不需要部署超时任务
        if !meta.is_ack_eliciting {
            return;
        }

        let rto = self.ctx.estimator.read().unwrap().rto();
        let listeners = self.lost_listeners.clone();
        let timeout_meta = meta.clone();
        // eprintln!("rto = {:?}", rto);
        let guard = actix_rt::spawn(async move {
            actix_rt::time::sleep(rto).await;

            // eprintln!("{:?} lost, resending", timeout_meta.packet_num);

            for listener in listeners {
                listener.do_send(LostBcast(timeout_meta.clone()));
            }
        })
        .into();

        self.packets.insert(meta.packet_num, (meta, guard));
    }
}

impl Handler<Ack> for Inflight {
    type Result = ();

    /// 接收到ack frame时，将其对应的packet从inflight中移除，并取消超时任务；必要时更新RTT
    fn handle(&mut self, Ack { frame, instant }: Ack, _ctx: &mut Self::Context) -> Self::Result {
        self.packets.retain(|_, (_, guard)| !guard.is_finished());

        let largest = frame.largest_ack;
        let delay = frame.delay;
        let spans: AckSpans = frame.into();

        for (pn, (_, guard)) in &self.packets {
            if spans.contains(*pn) {
                guard.abort();
            }
        }

        // 根据被确认的最大packet number的packet的发送时间来估算RTT
        // 只有最大packet number的packet是此次新ack时，才利用该packet来估算RTT
        if let Some((PacketMeta { sent, .. }, _)) = self.packets.get(&largest) {
            let rtt = instant - *sent;
            self.ctx.estimator.write().unwrap().update(delay, rtt);
        }

        // 根据ack frame来确认当前infight的packet中哪些已经被ack，并广播
        let acked: Vec<_> = self
            .packets
            .extract_if(|&pn, _| spans.contains(pn))
            .map(|(_, (meta, _))| meta)
            .collect();

        for listener in &self.acked_listeners {
            listener.do_send(AckedBcast(acked.clone()));
        }
    }
}

impl Handler<ListenAckedBcast> for Inflight {
    type Result = ();

    fn handle(&mut self, ListenAckedBcast(listener): ListenAckedBcast, _ctx: &mut Self::Context) {
        self.acked_listeners.push(listener);
    }
}

impl Handler<ListenLostBcast> for Inflight {
    type Result = ();

    fn handle(&mut self, ListenLostBcast(listener): ListenLostBcast, _ctx: &mut Self::Context) {
        self.lost_listeners.push(listener);
    }
}

/// 收到新的ack frame
#[derive(Message)]
#[rtype(result = "()")]
pub struct Ack {
    pub frame: AckFrame,
    pub instant: Instant,
}

/// 有新的packet被发出
#[derive(Message)]
#[rtype(result = "()")]
pub struct Sent(pub PacketMeta);
