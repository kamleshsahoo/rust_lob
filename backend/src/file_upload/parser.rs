use std::{fmt, str::FromStr, time::{Duration, Instant}};
use rust_decimal::Decimal;

use crate::engine::orderbook::BidOrAsk;
use super::upload::FileUploadOrderType;

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
  order: FileUploadOrderType
}

impl FileUploadOrder {
  fn parse(line: &str) -> Result<Self, ParseError> {
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


pub fn parse_file_orders(contents: String) -> (Vec<FileUploadOrderType>, Duration, i32, i32) {
  
  let mut total_raw_orders = 0;
  let mut invalid_orders = 0;
  let mut parsed_orders: Vec<FileUploadOrderType> = vec![];

  let start = Instant::now();
  for order in contents.lines() {
    total_raw_orders += 1;
    match FileUploadOrder::parse(order) {
      Ok(parsed_order) => {
        parsed_orders.push(parsed_order.order);
      },
      Err(e) => {
        invalid_orders += 1;
      }
    }
  }
  let parse_duration = start.elapsed();

  (parsed_orders, parse_duration, total_raw_orders, invalid_orders)
}