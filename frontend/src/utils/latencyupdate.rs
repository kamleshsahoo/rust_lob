// use std::time::Duration;
use tokio::time::Duration;

pub struct LatencyProcessor {
  latency_buffer: Vec<u128>,
  update_interval: tokio::time::Interval,
  window_size: usize
}

impl LatencyProcessor {
  pub fn new() -> Self {
    LatencyProcessor {
      latency_buffer: Vec::with_capacity(1_000), 
      update_interval: tokio::time::interval(Duration::from_millis(15)),
      window_size: 1_000
    }
  }

  pub fn process_latency(&mut self, latency: u128) {
    self.latency_buffer.push(latency)
  }

  pub fn get_latency_update(&mut self) -> Option<Vec<u128>> {
    if self.latency_buffer.is_empty() {
      return None;
    }
    let current_latency_buffer_len = self.latency_buffer.len();
    if current_latency_buffer_len > self.window_size {
      self.latency_buffer.drain(..current_latency_buffer_len - self.window_size);
    }
    Some(self.latency_buffer.clone())
  }

  pub async fn should_update(&mut self) -> bool {
    let _instant = self.update_interval.tick().await;
    true
  }
}