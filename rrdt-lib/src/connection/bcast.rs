use crate::packet::PacketMeta;
use actix::prelude::*;

/// 有新的packet被确认
#[derive(Message)]
#[rtype(result = "()")]
pub struct AckedBcast(pub Vec<PacketMeta>);

/// 注册`AckedBcast`的监听者
///
/// 广播的发送方需要处理该消息，将其加入到`acked_listeners`中
#[derive(Message)]
#[rtype(result = "()")]
pub struct ListenAckedBcast(pub Recipient<AckedBcast>);

/// 有新的packet丢失
#[derive(Message)]
#[rtype(result = "()")]
pub struct LostBcast(pub PacketMeta);

#[derive(Message)]
#[rtype(result = "()")]
pub struct ListenLostBcast(pub Recipient<LostBcast>);
