use super::constant::INITIAL_RTT;
use std::{cmp::min, time::Duration};

/// 基于RFC6298的RTT估计器
pub struct RttEstimator {
    latest: Duration,
    smoothed: Option<Duration>,
    var: Duration,
    min: Duration,
    max_ack_delay: Duration,
}

impl RttEstimator {
    pub fn new(max_ack_delay: Duration) -> Self {
        let initial_rtt = Duration::from_millis(INITIAL_RTT);
        Self {
            latest: initial_rtt,
            smoothed: None,
            var: initial_rtt / 2,
            min: initial_rtt,
            max_ack_delay,
        }
    }

    pub fn rtt(&self) -> Duration {
        self.smoothed.unwrap_or(self.latest)
    }

    /// RTO = smoothed_rtt + max(4 * rttvar, kGranularity) + ack_delay
    pub fn rto(&self) -> Duration {
        self.rtt() + Duration::from_micros(4 * self.var.as_micros() as u64) + self.max_ack_delay
    }

    pub(crate) fn update(&mut self, ack_delay: Duration, rtt: Duration) {
        self.latest = rtt;

        self.min = min(self.min, self.latest);

        if let Some(smoothed) = self.smoothed {
            let adjusted_rtt = if self.min + ack_delay <= self.latest {
                self.latest - ack_delay
            } else {
                self.latest
            };
            let var_sample = if smoothed > adjusted_rtt {
                smoothed - adjusted_rtt
            } else {
                adjusted_rtt - smoothed
            };
            self.var = (3 * self.var + var_sample) / 4;
            self.smoothed = Some((7 * smoothed + adjusted_rtt) / 8);
        } else {
            self.smoothed = Some(self.latest);
            self.var = self.latest / 2;
            self.min = self.latest;
        }
    }
}
