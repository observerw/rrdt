use super::Connection;
use crate::{
    packet::{HandshakePacket, MAX_PACKET_SIZE},
    serializable::Serializable,
};
use bytes::BytesMut;
use std::sync::Arc;
use tokio::{
    io,
    net::{ToSocketAddrs, UdpSocket},
};

pub struct ConnectionListener {
    socket: Arc<UdpSocket>,
}

impl ConnectionListener {
    pub async fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind(addr).await?);
        Ok(Self { socket })
    }

    pub async fn accept(&self) -> io::Result<Connection> {
        let mut buf = BytesMut::zeroed(MAX_PACKET_SIZE);
        let n = self.socket.recv(&mut buf).await?;
        let packet = HandshakePacket::decode(&mut &buf[..n]);
        let _params = packet.into_params();

        todo!()
    }
}
