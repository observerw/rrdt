use self::{
    bcast::{ListenAckedBcast, ListenLostBcast},
    packetizer::Packetizer,
    stream::{RecvStream, SendStream},
    streams::Streams,
};
use crate::{
    congestion::{rtt_estimator::RttEstimator, NewReno},
    connection::{ack_sender::AckSender, inflight::Inflight, receiver::Receiver, sender::Sender},
    packet::{CompressedPacket, HandshakePacket, LongPacket, MAX_PACKET_SIZE},
    serializable::Serializable,
    types::ConnectionId,
};
use actix::prelude::*;
use std::sync::{Arc, RwLock};
use tokio::{
    io,
    net::{ToSocketAddrs, UdpSocket},
};

pub use transport::{CompressedParams, TransportParams};

mod ack_sender;
mod bcast;
mod constant;
mod inflight;
mod listener;
mod packetizer;
mod receiver;
mod sender;
mod stream;
mod streams;
mod transport;

pub struct Connection {
    id: ConnectionId,
    addrs: Addrs,
    streams: Streams,
}

impl Connection {
    pub(crate) async fn with_socket(
        socket: Arc<UdpSocket>,
        params: TransportParams,
    ) -> io::Result<Self> {
        let id = rand::random();
        let estimator = Arc::new(RwLock::new(RttEstimator::new(params.max_ack_delay)));
        let congestion = Arc::new(RwLock::new(NewReno::default()));
        let ctx = ConnectionContext {
            id,
            socket,
            estimator,
            congestion,
            params,
        };

        let inflight = Inflight::new(ctx.clone()).start();
        let sender = Sender::new(
            ctx.clone(),
            sender::Addrs {
                inflight: inflight.clone(),
            },
        )
        .start();

        let packetizer = Packetizer::new(
            ctx.clone(),
            packetizer::Addrs {
                sender: sender.clone(),
            },
        )
        .start();

        let ack_sender = AckSender::new(
            ctx.clone(),
            ack_sender::Addrs {
                packetizer: packetizer.clone(),
            },
        )
        .start();

        let streams = Streams::new(
            ctx.clone(),
            stream::Addrs {
                packetizer: packetizer.clone(),
            },
        );

        let receiver = Receiver::new(
            ctx.clone(),
            receiver::Addrs {
                inflight: inflight.clone(),
                ack_sender: ack_sender.clone(),
                streams: streams.inner().clone(),
            },
        )
        .start();

        inflight.do_send(ListenAckedBcast(sender.clone().recipient()));
        inflight.do_send(ListenAckedBcast(streams.inner().clone().recipient()));
        inflight.do_send(ListenLostBcast(sender.clone().recipient()));
        inflight.do_send(ListenLostBcast(streams.inner().clone().recipient()));

        let addrs = Addrs { receiver };

        Ok(Self { id, addrs, streams })
    }

    pub async fn open(&mut self) -> SendStream {
        self.streams.open().await
    }

    pub async fn accept(&mut self) -> Option<RecvStream> {
        self.streams.accept().await
    }

    pub async fn close(self) {
        self.streams.close().await;
        println!("{} dropped", self.id);
    }

    pub fn id(&self) -> ConnectionId {
        self.id
    }
}

#[derive(Clone)]
pub struct ConnectionContext {
    id: ConnectionId,
    socket: Arc<UdpSocket>,
    estimator: Arc<RwLock<RttEstimator>>,
    congestion: Arc<RwLock<NewReno>>,
    params: TransportParams,
}

struct Addrs {
    receiver: Addr<Receiver>,
}

pub struct ConnectionListener {
    socket: Arc<UdpSocket>,
    params: TransportParams,
}

impl ConnectionListener {
    pub async fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind(addr).await?);
        let params = TransportParams::default();
        Ok(Self { socket, params })
    }

    pub fn with_params(mut self, params: TransportParams) -> Self {
        self.params = params;
        self
    }

    pub async fn accept(&self) -> io::Result<Connection> {
        let mut buf = [0u8; MAX_PACKET_SIZE];

        let (n, addr) = self.socket.recv_from(&mut buf).await?;
        let params = HandshakePacket::decode(&mut &buf[..n]).into_params();

        self.socket.connect(addr).await?;

        let packet = HandshakePacket::new(self.params.clone());
        let len = packet.len();
        packet.encode(&mut &mut buf[..]);
        let _ = self.socket.send(&buf[..len]).await?;

        Connection::with_socket(self.socket.clone(), params).await
    }
}

pub struct CompressConnectionListener {
    socket: Arc<UdpSocket>,
    params: Option<ConnectionListenerParams>,
}

impl CompressConnectionListener {
    pub async fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind(addr).await?);
        Ok(Self {
            socket,
            params: None,
        })
    }

    pub fn with_transport_params(mut self, params: TransportParams) -> Self {
        self.params = Some(ConnectionListenerParams::Transport(params));
        self
    }

    pub fn with_compressed_params(mut self, params: CompressedParams) -> Self {
        self.params = Some(ConnectionListenerParams::Compress(params));
        self
    }

    pub async fn accept(&self) -> io::Result<Option<Connection>> {
        let mut buf = [0u8; MAX_PACKET_SIZE];

        let (n, addr) = self.socket.recv_from(&mut buf).await?;
        let client_params = match LongPacket::decode(&mut &buf[..n]) {
            LongPacket::Handshake(packet) => packet.into_params(),
            _ => panic!("unexpected packet"),
        };

        self.socket.connect(addr).await?;

        match &self.params {
            Some(ConnectionListenerParams::Transport(params)) => {
                let packet = LongPacket::Handshake(HandshakePacket::new(params.clone()));
                let len = packet.len();
                packet.encode(&mut &mut buf[..]);
                let _ = self.socket.send(&buf[..len]).await?;

                let conn = Connection::with_socket(self.socket.clone(), client_params).await?;
                Ok(Some(conn))
            }
            Some(ConnectionListenerParams::Compress(params)) => {
                let packet = LongPacket::Compressed(CompressedPacket::new(params.clone()));
                let len = packet.len();
                packet.encode(&mut &mut buf[..]);
                let _ = self.socket.send(&buf[..len]).await?;

                Ok(None)
            }
            None => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "no params specified",
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConnectionListenerParams {
    Transport(TransportParams),
    Compress(CompressedParams),
}

pub struct ConnectionBuilder {
    socket: Arc<UdpSocket>,
    params: TransportParams,
}

impl ConnectionBuilder {
    pub async fn connect(
        local: impl ToSocketAddrs,
        remote: impl ToSocketAddrs,
    ) -> io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind(local).await?);
        socket.connect(remote).await?;

        let params = TransportParams::default();
        Ok(Self { socket, params })
    }

    pub fn with_params(mut self, params: TransportParams) -> Self {
        self.params = params;
        self
    }

    pub async fn build(self) -> io::Result<Connection> {
        let mut buf = [0u8; MAX_PACKET_SIZE];

        let packet = HandshakePacket::new(self.params);
        let len = packet.len();
        packet.encode(&mut &mut buf[..]);
        let _ = self.socket.send(&buf[..len]).await?;

        let n = self.socket.recv(&mut buf).await?;
        let params = HandshakePacket::decode(&mut &buf[..n]).into_params();

        Connection::with_socket(self.socket, params).await
    }
}

pub struct CompressConnectionBuilder {
    socket: Arc<UdpSocket>,
    params: TransportParams,
}

impl CompressConnectionBuilder {
    pub async fn connect(
        local: impl ToSocketAddrs,
        remote: impl ToSocketAddrs,
    ) -> io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind(local).await?);
        socket.connect(remote).await?;

        let params = TransportParams::default();
        Ok(Self { socket, params })
    }

    pub fn with_params(mut self, params: TransportParams) -> Self {
        self.params = params;
        self
    }

    pub async fn build(self) -> io::Result<ConnectionBuildResult> {
        let mut buf = [0u8; MAX_PACKET_SIZE];

        let packet = LongPacket::Handshake(HandshakePacket::new(self.params));
        let len = packet.len();
        packet.encode(&mut &mut buf[..]);
        let _ = self.socket.send(&buf[..len]).await?;

        let n = self.socket.recv(&mut buf).await?;
        let packet = LongPacket::decode(&mut &buf[..n]);

        match packet {
            LongPacket::Handshake(packet) => {
                let params = packet.into_params();
                let conn = Connection::with_socket(self.socket.clone(), params).await?;

                Ok(ConnectionBuildResult::Connection(conn))
            }
            LongPacket::Compressed(packet) => {
                let params = packet.into_params();
                Ok(ConnectionBuildResult::Compressed(params))
            }
            _ => panic!("unexpected packet"),
        }
    }
}

pub enum ConnectionBuildResult {
    Connection(Connection),
    Compressed(CompressedParams),
}
