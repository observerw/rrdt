use super::bcast::{AckedBcast, LostBcast};
use super::stream::{recv_stream, send_stream, RecvStream, SendStream};
use super::{packetizer, stream, ConnectionContext};
use crate::frame::stream::{
    MaxStreamDataFrame, MaxStreamDataMeta, StreamDataFrame, StreamDataMeta,
};
use crate::frame::{Frame, FrameMeta, StreamFrame};
use crate::packet::PacketMeta;
use crate::serializable::Serializable;
use crate::types::{InfReceiver, InfSender, StreamId};
use crate::utils::choice::Choice;
use actix::prelude::*;
use futures::future::join_all;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct StreamsInner {
    ctx: ConnectionContext,
    addrs: stream::Addrs,
    next_id: StreamId,
    // map: HashMap<StreamId, Stream>,
    send_map: HashMap<StreamId, SendStream>,
    recv_map: HashMap<StreamId, RecvStream>,

    accept_handle: InfSender<RecvStream>,
}

impl StreamsInner {
    pub fn new(
        ctx: ConnectionContext,
        addrs: stream::Addrs,
        accept_handle: InfSender<RecvStream>,
    ) -> Self {
        Self {
            ctx,
            addrs,
            send_map: HashMap::new(),
            recv_map: HashMap::new(),
            next_id: 0,
            accept_handle,
        }
    }

    fn get_send(&mut self, id: StreamId) -> &SendStream {
        self.send_map
            .entry(id)
            .or_insert_with(|| SendStream::new(id, self.addrs.clone()))
    }

    /// 远端打开的stream，会被放入`accept_queue`中
    fn get_recv(&mut self, id: StreamId) -> &RecvStream {
        self.recv_map.entry(id).or_insert_with(|| {
            let stream = RecvStream::new(id, self.addrs.clone());
            let _ = self.accept_handle.send(stream.clone());
            stream
        })
    }
}

impl Actor for StreamsInner {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let estimator = self.ctx.estimator.clone();
        let congestion = self.ctx.congestion.clone();

        ctx.run_interval(Duration::from_millis(1), move |_, ctx| {
            let mut window = congestion.read().unwrap().window();
            window += window / 4;

            let rtt = estimator.read().unwrap().rtt().as_micros() as u64;
            let bytes = (window * 1000 / rtt) as usize;

            ctx.notify(Send { bytes });
        });
    }
}

impl Handler<AckedBcast> for StreamsInner {
    type Result = ();

    fn handle(&mut self, AckedBcast(meta): AckedBcast, _ctx: &mut Self::Context) -> Self::Result {
        for PacketMeta { frame_meta, .. } in meta {
            for meta in frame_meta {
                match meta {
                    // stream frame被ack时将send window中的对应部分标记为ack
                    FrameMeta::Stream(StreamDataMeta { id, range }) => {
                        let stream = self.get_send(id);
                        stream.inner().do_send(send_stream::Ack(range));
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Handler<LostBcast> for StreamsInner {
    type Result = ();

    fn handle(
        &mut self,
        LostBcast(PacketMeta { frame_meta, .. }): LostBcast,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        for meta in frame_meta {
            match meta {
                // stream frame丢失时将send window中的对应部分标记为retransmit
                FrameMeta::Stream(StreamDataMeta { id, range }) => {
                    let stream = self.get_send(id);
                    stream.inner().do_send(send_stream::Retransmit(range));
                }
                // max stream data frame丢失时立即更新一次recv window
                FrameMeta::MaxStreamData(MaxStreamDataMeta { id }) => {
                    let stream = self.get_recv(id);
                    stream.inner().do_send(recv_stream::Update);
                }
            }
        }
    }
}

impl Handler<Dispatch> for StreamsInner {
    type Result = ();

    /// 将stream相关的frame分发到对应的stream
    fn handle(&mut self, Dispatch(frame): Dispatch, _ctx: &mut Self::Context) -> Self::Result {
        match frame {
            StreamFrame::Data(StreamDataFrame {
                id,
                offset,
                data,
                fin,
            }) => {
                let stream = self.get_recv(id);
                stream
                    .inner()
                    .do_send(recv_stream::Write { offset, data, fin });
            }
            StreamFrame::MaxData(MaxStreamDataFrame { id, max_data, .. }) => {
                let stream = self.get_send(id);
                stream.inner().do_send(send_stream::MaxData(max_data));
            }
        }
    }
}

impl Handler<Send> for StreamsInner {
    type Result = ();

    /// 从所有stream中以随机顺序遍历读取指定长度的数据，将读取到的数据组成`StreamDataFrame`送入发送队列
    fn handle(&mut self, Send { mut bytes }: Send, ctx: &mut Self::Context) -> Self::Result {
        if self.send_map.is_empty() {
            return;
        }

        let streams: Vec<_> = self.send_map.values().cloned().collect();
        let packetizer = self.addrs.packetizer.clone();
        ctx.spawn(
            async move {
                for stream in streams.choice() {
                    let frame = stream
                        .inner()
                        .send(send_stream::Read { bytes })
                        .await
                        .unwrap();

                    if let Ok(Some(frame)) = frame {
                        bytes -= frame.len();
                        let _ = packetizer.do_send(packetizer::Send(Frame::Stream(frame)));
                    }

                    if bytes <= StreamDataFrame::min_len() {
                        break;
                    }
                }
            }
            .into_actor(self),
        );
    }
}

impl Handler<Open> for StreamsInner {
    type Result = Response<SendStream>;

    fn handle(&mut self, _: Open, _ctx: &mut Self::Context) -> Self::Result {
        let stream = self.get_send(self.next_id).to_owned();
        self.next_id += 1;
        Response::reply(stream)
    }
}

impl Handler<Close> for StreamsInner {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, _: Close, _ctx: &mut Self::Context) -> Self::Result {
        let recvs: Vec<_> = self.recv_map.values().cloned().collect();
        let sends: Vec<_> = self.send_map.values().cloned().collect();

        Box::pin(async move {
            join_all(sends.into_iter().map(|stream| stream.close())).await;
            join_all(recvs.into_iter().map(|stream| stream.close())).await;

            // join!(
            //     join_all(sends.into_iter().map(|stream| stream.close())),
            //     join_all(recvs.into_iter().map(|stream| stream.close()))
            // );
        })
    }
}

/// 主动打开下一个新的stream
///
/// 主动打开的stream不会进入到`accept_queue`中
#[derive(Message)]
#[rtype(result = "SendStream")]
pub struct Open;

/// 将stream相关的frame分发到对应的stream
#[derive(Message)]
#[rtype(result = "()")]
pub struct Dispatch(pub StreamFrame);

/// 从所有stream中读取指定长度的数据，将读取到的数据组成`StreamDataFrame`送入发送队列
#[derive(Message)]
#[rtype(result = "()")]
pub struct Send {
    bytes: usize,
}

/// 关闭所有stream
#[derive(Message)]
#[rtype(result = "()")]
pub struct Close;

/// 对外暴露的streams接口，用于获取对端开启的stream或主动开启新的stream
pub struct Streams {
    ctx: ConnectionContext,
    inner: Addr<StreamsInner>,
    accept_queue: InfReceiver<RecvStream>,

    recv_count: u16,
}

impl Streams {
    pub fn new(ctx: ConnectionContext, addrs: stream::Addrs) -> Self {
        let (accept_handle, accept_queue) = mpsc::unbounded_channel();
        let inner = StreamsInner::new(ctx.clone(), addrs, accept_handle).start();

        Self {
            ctx,
            inner,
            accept_queue,
            recv_count: 0,
        }
    }

    /// 主动打开下一个新的stream
    pub async fn open(&mut self) -> SendStream {
        self.inner.send(Open).await.unwrap()
    }

    /// 等待获取下一个对端开启的stream
    pub async fn accept(&mut self) -> Option<RecvStream> {
        // 开启数量达到了对端承诺的数量，则不再接受新的stream
        if self.recv_count == self.ctx.params.streams {
            None
        } else {
            let stream = self.accept_queue.recv().await.unwrap();
            self.recv_count += 1;
            Some(stream)
        }
    }

    pub async fn close(self) {
        let _ = self.inner.send(Close).await.unwrap();
    }

    pub(crate) fn inner(&self) -> &Addr<StreamsInner> {
        &self.inner
    }
}
