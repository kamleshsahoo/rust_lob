use std::{collections::HashMap, fmt, str::FromStr, sync::Arc, time::Duration};
use dioxus::logger::tracing::info;
use dioxus::{logger::tracing::warn, prelude::*};
use dioxus::html::FileEngine;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};



#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct FinalStats {
  pub total_time: Duration,
  pub avl_rebalances: i64,
  pub executed_orders_cnt: i64,
  pub nos: i64
}

pub fn format_duration(duration: Duration) -> String {
  let nanos = duration.as_nanos();

  if nanos >= 1_000_000_000 {
    // seconds
    let seconds = duration.as_secs_f64();
    format!("{:.3} s", seconds)
  } else if nanos >= 1_000_000 {
    // milliseconds
    let millis = duration.as_secs_f64() * 1_000.0;
    format!("{:.3} ms", millis)    
  } else if nanos >= 1_000 {
    // microseconds
    let micros = duration.as_secs_f64() * 1_000_000.0;
    format!("{:.0} μs", micros)    
  } else {
    // nanoseconds
    format!("{} ns", nanos)    
  }
}

impl FinalStats {
  pub fn add(&self, other: &FinalStats) -> FinalStats {
    FinalStats {
      total_time: self.total_time + other.total_time,
      avl_rebalances: self.avl_rebalances + other.avl_rebalances,
      executed_orders_cnt: self.executed_orders_cnt + other.executed_orders_cnt,
      nos: self.nos + other.nos
    }
  }
}

#[derive(Debug, Deserialize)]
pub struct UploadResponse {
  // success: bool,
  pub data: Option<HashMap<String, FinalStats>>,
  pub session_id: Option<String>,
  // error_msg: Option<String>  
}

#[derive(Debug, Deserialize)]
pub struct LargeUploadResponse {
  pub orderbook_results: HashMap<String, FinalStats>,
  pub parse_results: (Duration, i32, i32)
}

#[derive(Debug, Serialize)]
pub struct UploadRequest {
  pub session_id: Option<String>,
  pub total_chunks: usize,
  pub total_orders: usize,
  pub chunk_number: usize,
  pub orders: Vec<FileUploadOrderType>
}


pub struct PreviewRow {
  pub row_id: String,
  pub ordertype: String,
  pub order_id: String,
  pub side: String,
  pub shares: String,
  pub price: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum BidOrAsk {
  Bid,
  Ask,
}

impl fmt::Display for BidOrAsk {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
        Self::Bid => write!(f, "BID"),
        Self::Ask => write!(f, "ASK"),
    }
  }
}

impl FromStr for BidOrAsk {
  type Err = ParseError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
      match s.to_lowercase().as_str() {
        "bid" => Ok(BidOrAsk::Bid),
        "ask" => Ok(BidOrAsk::Ask),
        _ => Err(ParseError::InvalidBidorAsk(s.to_string())) 
      }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum FileUploadOrderType {
  Add {
    id: u64,
    side: BidOrAsk,
    shares: u64,
    price: Decimal
  },
  Modify {
    id: u64,
    shares: u64,
    price: Decimal
  },
  Cancel {
    id: u64,
  },
}

#[derive(Debug)]
pub enum ParseError {
  InvalidBidorAsk(String),
  InvalidOrderType(String),
  InvalidOrderFormat(String),
  InvalidOrderId(std::num::ParseIntError),
  InvalidShares(std::num::ParseIntError),
  InvalidPrice(rust_decimal::Error),
  Empty
}

impl fmt::Display for ParseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidBidorAsk(bid_or_ask) => {
        write!(f, "Invalid bid/ask string: {}", bid_or_ask)
      },
      Self::InvalidOrderType(order_type) => {
        write!(f, "Invalid order type string: {}", order_type)
      },
      Self::InvalidOrderFormat(order) => {
        write!(f, "Invalid {} order format", order)
      },
      Self::InvalidOrderId(err) => {
        write!(f, "Faled to parse Order ID: {:?}", err)
      },
      Self::InvalidShares(err) => {
        write!(f, "Faled to parse shares: {:?}", err)
      },
      Self::InvalidPrice(err) => {
        write!(f, "Faled to parse price: {:?}", err)
      }
      Self::Empty => {
        write!(f, "Empty order line in file")
      }
    }
  }
}

impl std::error::Error for ParseError {}

impl From<std::num::ParseIntError> for ParseError {
  fn from(value: std::num::ParseIntError) -> Self {
      ParseError::InvalidOrderId(value)
  }
}

impl From<rust_decimal::Error> for ParseError {
  fn from(value: rust_decimal::Error) -> Self {
      ParseError::InvalidPrice(value)
  }
}

pub struct FileUploadOrder {
  pub order: FileUploadOrderType
}

impl FileUploadOrder {
  pub fn parse(line: &str) -> Result<Self, ParseError> {
    let parts: Vec<&str> = line.split(|c| c == ',').map(|s| s.trim()).collect();

    let order_type = match parts.get(0).map(|s| s.to_uppercase()) {
        Some(s) => s,
        None => return Err(ParseError::Empty)
    };

    let order = match order_type.as_str() {
      "ADD" => {
        if parts.len() != 5 {
          return Err(ParseError::InvalidOrderFormat("ADD".to_string()));
        }
        let id = parts[1].parse().map_err(|err| ParseError::InvalidOrderId(err))?;
        let side = BidOrAsk::from_str(parts[2])?;
        let shares = parts[3].parse().map_err(|err| ParseError::InvalidShares(err))?;
        let mut price =  Decimal::from_str(parts[4])?; 
        price.rescale(2);
        
        FileUploadOrderType::Add { 
          id,
          side,
          shares,
          price
        }
      },
      "MODIFY" => {
        if parts.len() != 4 {
          return Err(ParseError::InvalidOrderFormat("MODIFY".to_string()));
        }
        let id = parts[1].parse().map_err(|err| ParseError::InvalidOrderId(err))?;
        let shares = parts[2].parse().map_err(|err| ParseError::InvalidShares(err))?;
        let mut price =  Decimal::from_str(parts[3])?;
        price.rescale(2); 

        FileUploadOrderType::Modify { 
          id,
          shares,
          price
        }
      },
      "CANCEL" => {
        if parts.len() != 2 {
          return Err(ParseError::InvalidOrderFormat("CANCEL".to_string()));
        }
        let id = parts[1].parse().map_err(|err| ParseError::InvalidOrderId(err))?;
        FileUploadOrderType::Cancel { id }
      },
      _ => return Err(ParseError::InvalidOrderType(order_type)),
    };
    Ok(FileUploadOrder {order})
  }
}

// pub async fn read_files(file_engine: Arc<dyn FileEngine>, mut parsed_orders: Signal<Vec<FileUploadOrderType>>,
// mut total_raw_orders: Signal<i32>, mut invalid_orders: Signal<i32>) {
//   let files = file_engine.files();
//   for file_name in &files {
//     let size = file_engine.file_size(&file_name).await.unwrap();
//     info!("file size for {:?}: {:?}", &file_name, size);
//     if let Some(contents) = file_engine.read_file_to_string(&file_name).await {
//       // info!("contents: {}", &contents);
//       for order in contents.lines() {
//         // info!("order: {}", order);
//         *total_raw_orders.write() += 1;
//         match FileUploadOrder::parse(order) {
//           Ok(valid_order) => {
//             parsed_orders.write().push(valid_order.order);
//           }
//           Err(e) => {
//             warn!("Parse error: {:?}", e);
//             *invalid_orders.write() += 1;
//           }
//         }
//       }
//     }
//   }
// }