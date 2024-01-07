use bytes::Bytes;
use tokio::sync::{broadcast, mpsc, oneshot};

pub type Sender<T> = mpsc::Sender<T>;
pub type Receiver<T> = mpsc::Receiver<T>;

pub type InfSender<T> = mpsc::UnboundedSender<T>;
pub type InfReceiver<T> = mpsc::UnboundedReceiver<T>;

pub type Responder<T> = oneshot::Sender<T>;
pub type Requester<T> = oneshot::Receiver<T>;

pub type BcastSender<T> = broadcast::Sender<T>;
pub type BcastReceiver<T> = broadcast::Receiver<T>;

pub type PacketNum = u64;
pub type StreamId = u16;
pub type ConnectionId = u64;
pub type Offset = u64;
