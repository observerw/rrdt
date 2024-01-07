mod constant;
pub mod rtt_estimator;

use self::constant::{BASE_DATAGRAM_SIZE, DEFAULT_LOSS_REDUCTION_FACTOR};
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct NewReno {
    config: NewRenoConfig,
    current_mtu: u64,

    /// 拥塞窗口，也即同一时间内允许正在传输的最大数据量
    window: u64,

    /// 慢启动阈值，当拥塞窗口小于ssthresh时，处于慢启动状态，拥塞窗口增长的速度为已确认的数据量
    ///
    /// 当拥塞窗口大于ssthresh时，处于拥塞避免状态，拥塞窗口增长的速度为已确认的数据量除以拥塞窗口大小
    ssthresh: u64,

    /// 第一次检测到丢包时的时间，当收到一个在这个时间之后发送的数据包的确认时，退出恢复状态
    recovery_start_time: Instant,

    /// 在离开慢启动状态后，已被对端确认的数据量
    bytes_acked: u64,
}

impl NewReno {
    pub fn new(config: NewRenoConfig, now: Instant, current_mtu: u16) -> Self {
        Self {
            window: config.initial_window,
            ssthresh: u64::max_value(),
            recovery_start_time: now,
            current_mtu: current_mtu as u64,
            config,
            bytes_acked: 0,
        }
    }

    pub fn on_ack(&mut self, sent: Instant, bytes: u64) {
        if sent <= self.recovery_start_time {
            return;
        }

        if self.window < self.ssthresh {
            // 慢启动
            self.window += bytes;

            if self.window >= self.ssthresh {
                // 退出慢启动
                self.bytes_acked = self.window - self.ssthresh;
            }
        } else {
            // 拥塞避免
            self.bytes_acked += bytes;

            if self.bytes_acked >= self.window {
                self.bytes_acked -= self.window;
                self.window += self.current_mtu;
            }
        }
    }

    /// 发生了丢包
    pub fn on_loss(&mut self, now: Instant, sent: Instant, _bytes: u64) {
        if sent <= self.recovery_start_time {
            return;
        }

        self.recovery_start_time = now;
        self.window = (self.window as f32 * self.config.loss_reduction_factor) as u64;
        self.window = self.window.max(self.minimum_window());
        self.ssthresh = self.window;
    }

    pub fn window(&self) -> u64 {
        self.window
    }

    pub fn initial_window(&self) -> u64 {
        self.config.initial_window
    }

    pub fn minimum_window(&self) -> u64 {
        2 * self.current_mtu
    }
}

impl Default for NewReno {
    fn default() -> Self {
        let config = NewRenoConfig::default();
        let now = Instant::now();
        let mtu = 1200;
        Self::new(config, now, mtu)
    }
}

#[derive(Debug, Clone)]
pub struct NewRenoConfig {
    pub initial_window: u64,
    pub loss_reduction_factor: f32,
}

impl Default for NewRenoConfig {
    fn default() -> Self {
        Self {
            initial_window: 14720.clamp(2 * BASE_DATAGRAM_SIZE, 10 * BASE_DATAGRAM_SIZE),
            loss_reduction_factor: DEFAULT_LOSS_REDUCTION_FACTOR,
        }
    }
}
