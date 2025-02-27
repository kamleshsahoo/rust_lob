use std::collections::{BTreeMap, HashMap};
use charming::datatype::{CompositeValue, DataPoint, NumericValue};
use dioxus::logger::tracing::info;
//use dioxus::logger::tracing::info;
use crate::pages::simulator::{EngineStats, ExecutedOrders};

/* Per order we get 
{order_type:"ADD"/"MODIFY"/"CANCEL", latency, avl_rebalances, executed_orders_cnt}
*/

fn mean(vec: &Vec<i64>) -> f64 {
  let n = vec.len();
  let mut sum = 0;
  
  for i in vec { 
    sum += *i;
  }
  (sum as f64)/ (n as f64)
}


fn quantiles_with_mean(vec: &mut Vec<i64>, qvals: &Vec<f64>) -> Vec<f64> {
  /*
  calculates a non-parametric estimate of the inv cumulative distributive function (cdf) using linear interpolation
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
  result.push(mean(vec));
  
  result
}

pub fn get_latency_by_ordertype<'a>(stats: &'a Vec<EngineStats>, qvals: &'a Vec<f64>) ->  HashMap<String, Vec<f64>> {

  let mut ordertype_fold = stats.iter().fold(HashMap::new(), |mut map_state, e| {
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
      _ => unreachable!("unsupported order type in engine stat!")
    }
    map_state
  });

  let ordertype_fold_stats: HashMap<String, Vec<f64>> = ordertype_fold.iter_mut().map(|(k, v)| { 
    let stats = quantiles_with_mean( v, qvals);
    (String::from(*k), stats)
  }).collect::<HashMap<_,_>>();

  ordertype_fold_stats

}

pub fn get_latency_by_avl_trades<'a>(stats: &'a Vec<EngineStats>) -> BTreeMap<(i64, i64), f64> {

  let trades_avl_fold = stats.iter().fold(
    BTreeMap::new(), |mut map_state, e| { 
      map_state
      .entry((e.executed_orders_cnt as i64, e.avl_rebalances))
      .or_insert(Vec::<i64>::new())
      .push(e.latency);
      map_state
    }
  );
  // may use filter_map
  let trades_avl_fold_stats:BTreeMap<(i64, i64), f64>  = trades_avl_fold.iter().map(|(k, v)| {
    let avg_latency = mean(v); 
    (*k, avg_latency)
  }).collect::<BTreeMap<_,_>>();
  
  trades_avl_fold_stats
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

pub fn bar3d_data(data: &BTreeMap<(i64, i64), f64>) -> Vec<Vec<CompositeValue>> {
  let mut data_3d: Vec<Vec<CompositeValue>> = vec![];
  // data_3d.push(vec![CompositeValue::String(String::from("Trades")), CompositeValue::String(String::from("AVL")), CompositeValue::String(String::from("Latency"))]);

  for (k, v) in data.iter() {
    let trade_cnt = CompositeValue::Number(NumericValue::Integer(k.0));
    let avl_cnt = CompositeValue::Number(NumericValue::Integer(k.1));
    let avg_latency = CompositeValue::Number(NumericValue::Integer(*v as i64));
    data_3d.push(vec![trade_cnt, avl_cnt, avg_latency]);
  }
  data_3d
}

pub fn get_cumlative_results(engine_stats: Vec<EngineStats>, executed_orders: Vec<ExecutedOrders>, qvals: Vec<f64>) -> (Vec<i64>, HashMap<String, Vec<f64>>, BTreeMap<(i64, i64), f64>) {
  info!("Simulation complete. Computing cumlative results");
  info!("engine stats vec len: {:?} trades: {:?}", engine_stats.len(), executed_orders.len());

  let latencies = engine_stats.iter().map(|e| e.latency).collect::<Vec<i64>>();
  let latencies_by_ordertype = get_latency_by_ordertype(&engine_stats, &qvals);
  let latencies_by_avl_trades = get_latency_by_avl_trades(&engine_stats);

  (latencies, latencies_by_ordertype, latencies_by_avl_trades)
}