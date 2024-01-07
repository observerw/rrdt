use super::packetizer::Packetizer;
use super::ConnectionContext;
use super::{constant::*, packetizer};
use crate::{
    frame::{
        ack::{AckFrame, AckSpans},
        Frame,
    },
    types::PacketNum,
};
use actix::prelude::*;
use std::time::Duration;
use tokio::time::Instant;

pub struct AckSender {
    ctx: ConnectionContext,
    addrs: Addrs,
    state: State,

    /// 当前已经收到的所有packet
    spans: AckSpans,

    /// 当前已经ack过的最大packet number
    ///
    /// 收到小于等于该值的ack_eliciting packet时，认为是乱序包，立即发送ack frame
    acked: PacketNum,

    /// 接收到多少顺序包后发送一次ack frame
    wait_count: u64,

    /// 收到第一个顺序后进入`Waiting`状态，部署一个超时任务
    ///
    /// 若在`max_ack_delay`时间内没有收到`wait_count`个顺序包，则说明有丢包，立即发送ack frame
    timeout_handle: Option<SpawnHandle>,
}

impl AckSender {
    pub fn new(ctx: ConnectionContext, addrs: Addrs) -> Self {
        Self {
            ctx,
            addrs,
            state: State::Idle,
            spans: AckSpans::new(),
            acked: 0,
            wait_count: DEFAULT_ACK_WAIT_COUNT,
            timeout_handle: None,
        }
    }

    pub fn with_wait_count(self, wait_count: u64) -> Self {
        Self { wait_count, ..self }
    }

    /// 以当前时刻为基准计算delay，发送ack frame
    fn send(&mut self, instant: Instant) {
        let delay = Instant::now() - instant;
        self.send_with_delay(delay);
    }

    /// 发送ack frame
    fn send_with_delay(&self, delay: Duration) {
        let mut frame: AckFrame = self.spans.clone().into();
        frame.set_delay(delay);

        self.addrs
            .packetizer
            .do_send(packetizer::Send(Frame::Ack(frame)));
    }
}

impl Actor for AckSender {
    type Context = Context<Self>;
}

impl Handler<Recv> for AckSender {
    type Result = ();

    fn handle(
        &mut self,
        Recv {
            packet_num,
            is_ack_eliciting,
            instant,
        }: Recv,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        self.spans.insert(packet_num);

        // 如果不是ack_eliciting的包，选择性更新acked即可
        if !is_ack_eliciting {
            self.acked = std::cmp::max(self.acked, packet_num);
            self.state = State::Idle;
            return;
        }

        match self.state {
            // 已发送过ack frame，且还没收到过顺序包
            State::Idle => {
                // 该包是顺序包，进入`Waiting`状态且部署超时任务
                if packet_num == self.acked + 1 {
                    self.timeout_handle =
                        Some(ctx.notify_later(Timeout, self.ctx.params.max_ack_delay));
                    self.state = State::Waiting(1);
                } else {
                    if packet_num >= self.acked + 2 {
                        self.acked = packet_num;
                    }

                    self.send(instant);
                    self.state = State::Idle;
                }
            }
            // 收到了`count`个顺序包
            State::Waiting(count) => {
                let now_count = count + 1;
                // 收到了顺序包
                if packet_num == self.acked + now_count
                    // 且还没达到wait_count
                    && now_count < self.wait_count
                {
                    self.state = State::Waiting(now_count);
                }
                // 收到了乱序包，或达到了wait_count
                else {
                    // 取消超时任务
                    if let Some(handle) = self.timeout_handle.take() {
                        ctx.cancel_future(handle);
                    }
                    self.send(instant);
                    self.state = State::Idle;
                }
            }
        }
    }
}

impl Handler<Timeout> for AckSender {
    type Result = ();

    fn handle(&mut self, _: Timeout, _ctx: &mut Self::Context) -> Self::Result {
        match self.state {
            State::Waiting(_) => {
                // 超时后立即发送ack frame，delay设置为`max_ack_delay`
                self.send_with_delay(self.ctx.params.max_ack_delay);
                self.state = State::Idle;
            }
            _ => {}
        }
    }
}

/// 接收到了新的packet
#[derive(Message)]
#[rtype(result = "()")]
pub struct Recv {
    pub packet_num: PacketNum,
    pub is_ack_eliciting: bool,
    pub instant: Instant,
}

/// packet没有在`max_ack_delay`时间内收到ack frame
#[derive(Message)]
#[rtype(result = "()")]
struct Timeout;

pub struct Addrs {
    pub packetizer: Addr<Packetizer>,
}

enum State {
    Idle,
    Waiting(u64),
}
