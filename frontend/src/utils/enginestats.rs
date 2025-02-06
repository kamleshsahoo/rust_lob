// use std::time::Duration;

/* TODO: remove if not needed
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
*/

/* Per order we get 
{order_type:"ADD"/"MODIFY"/"CANCEL", latency, avl_rebalances, executed_orders_cnt}
*/

use std::collections::HashMap;

use crate::EngineStats;

fn quantiles(vec: &mut Vec<u128>, qvals: &Vec<f64>) -> Vec<f64> {
  /*
  calculates a non-parametric estimate if the inv cumulative distributive function
  uses `linear` interpolation
  `(1-g)*y[j] + g*y[j+1]``
  where the index j and coefficient g are the integral and fractional components
  of q * (n-1), and n is the number of elements in the sample.
  NOTE: result includes average as the last element
  */

  let n = vec.len();
  vec.sort();
  let mut result: Vec<f64> = vec![];
  
  for q in qvals {
      let x = q*((n-1) as f64);
      let j = x.floor() as usize; //integral part 
      let g = x - x.floor();  //fractional part
      
      let part1 = (1.0-g)*(vec[j] as f64);
      let part2 = if j+1 > n-1 { g*vec[n-1] as f64 } else { g*vec[j+1] as f64 };

      result.push(part1 + part2);
  }
  
  // last element is average/mean
  let mut sum = 0;
  for i in vec {
      sum += *i;
  }
  result.push((sum as f64)/ (n as f64));
  
  result
}

pub fn get_latency_by_ordertype<'a>(stats: &'a Vec<EngineStats>, qvals: &'a Vec<f64>) -> HashMap<String, Vec<f64>> {
  
  let mut latency_fold = stats.iter().fold(HashMap::new(), |mut map_state, e| {
    match e.order_type.as_str() {
      "ADD" => { 
        map_state.entry("ADD").or_insert(Vec::<u128>::new()).push(e.latency)
      },
      "MODIFY" => {
        map_state.entry("MODIFY").or_insert(Vec::<u128>::new()).push(e.latency)
      },
      "CANCEL" => {
        map_state.entry("CANCEL").or_insert(Vec::<u128>::new()).push(e.latency)
      },
      _ => panic!("unsupported order type in engine stat!")
    }
    map_state
  });

  let latency_fold_stats = latency_fold.iter_mut().map(|(k, v)| { 
    let stats = quantiles(v, qvals);
    (String::from(*k), stats)
  }).collect::<HashMap<_,_>>();

  latency_fold_stats

}