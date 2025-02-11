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
use charming::datatype::{CompositeValue, DataPoint, NumericValue};
use crate::EngineStats;

fn quantiles(vec: &mut Vec<i64>, qvals: &Vec<f64>) -> Vec<f64> {
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
        map_state.entry("ADD").or_insert(Vec::<i64>::new()).push(e.latency)
      },
      "MODIFY" => {
        map_state.entry("MODIFY").or_insert(Vec::<i64>::new()).push(e.latency)
      },
      "CANCEL" => {
        map_state.entry("CANCEL").or_insert(Vec::<i64>::new()).push(e.latency)
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

#[derive(Debug)]
struct Bin {
    range: BinRange,
    count: usize,
}


#[derive(Debug)]
enum BinRange {
    Fixed(i64, i64),  // [lower, upper)
    CatchAll(i64),    // [max, inf)
}

pub fn bin_data(data: &Vec<i64>, bin_width: i64, max_value: i64) ->  Vec<DataPoint> {

    assert_eq!(false, data.is_empty(), "data for binning should not be empty!");

    let num_bins: i64 = max_value/bin_width;
    let mut bins = HashMap::new();

    for &value in data {
      if value >= max_value {
        *bins.entry(num_bins).or_insert(0) += 1;
        continue;
      }
      let bin_index = value/bin_width;
      *bins.entry(bin_index).or_insert(0) += 1;
    }

    let mut result = Vec::new();
    for (bin_index, count) in bins {
      let range = if bin_index == num_bins {
        BinRange::CatchAll(max_value)
    } else {
        let lower = bin_index * bin_width;
        let upper = lower + bin_width;
        BinRange::Fixed(lower, upper)
    };

    result.push(Bin { range, count });
    }

  result.sort_by_key(|bin| match bin.range {
    BinRange::Fixed(lower, _) | BinRange::CatchAll(lower) => lower
  });
 

  // let _u: Vec<_> = result.drain(num_bins as usize..).collect();
  // info!("excluded bin: {:?}", _u);
  
  let processed_bins = result.into_iter().filter_map(|bin| {
    match bin.range {
      BinRange::Fixed(x0, x1) => {
        let centre: f64 = (x0 + x1) as f64 / 2.0;
        let label = format!("{}-{}", x0, x1);
      
        Some(DataPoint::Value(CompositeValue::Array(vec![
          CompositeValue::Number(NumericValue::Float(centre)), 
          CompositeValue::Number(NumericValue::Integer(bin.count as i64)),
          CompositeValue::Number(NumericValue::Integer(x0)),
          CompositeValue::Number(NumericValue::Integer(x1)),
          CompositeValue::String(label)
        ])))      
        },
        BinRange::CatchAll(_) => None
      }
    }
  ).collect::<Vec<_>>();

  processed_bins
}