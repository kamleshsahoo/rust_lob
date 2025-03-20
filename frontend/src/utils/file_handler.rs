use dioxus::prelude::*;
use rmp_serde::Serializer;
use std::{collections::HashMap, fmt, io::Write, str::FromStr, time::Duration};
use flate2::{write::DeflateEncoder, Compression};
use reqwest::multipart::{Form, Part};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{auth::AuthSignature, server::{AppError, HealthCheckResponse, LargeUploadResponse, SmallUploadRequest, SmallUploadResponse}};

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
    format!("{:.2} ms", millis)    
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

// Upload Handler
pub struct UnifiedUploader {
  client: reqwest::Client,
  small_upload_url: String,
  large_upload_url: String,
  health_check_url: String,
  chunk_size: usize,
  compression_enabled: bool
}

impl UnifiedUploader {
  pub fn new(client: reqwest::Client, small_url: &str, large_url: &str, health_url: &str) -> Self {
    Self {
      client,
      small_upload_url: small_url.to_string(),
      large_upload_url: large_url.to_string(),
      health_check_url: health_url.to_string(),
      chunk_size: 8 * 1024 * 1024, // default chunk size of 8MB
      compression_enabled: false // compression in not enabled by default
    }
  }

  pub fn with_chunk_size(mut self, size: usize) -> Self {
    self.chunk_size = size;
    self
  }

  pub fn with_compression(mut self, enabled: bool) -> Self {
    self.compression_enabled = enabled;
    self
  }

  pub async fn check_health(&self) -> Result<(), AppError> {
    let resp = self.client.get(&self.health_check_url).send().await.map_err(|e| AppError::ServerUnhealthy(e.to_string()))?;
    let _json_resp = resp.json::<HealthCheckResponse>().await.map_err(|e| AppError::DeserializeError(e.to_string()))?;
    //info!("health check for upload succeeded with: {:?}", json_resp);
    Ok(())
  }

  pub async fn upload_large_file(&self,
    file_bytes: Vec<u8>,
    f_name: &str,
    auth_signer: AuthSignature,
    mut ob_results: Signal<Option<HashMap<String, FinalStats>>>,
    mut parse_results: Signal<Option<(Duration, i32, i32)>>
  ) -> Result<(), AppError> {
    let total_bytes = file_bytes.len();
    //info!("**large file total bytes: {}", &total_bytes);
    let total_chunks = (total_bytes + self.chunk_size - 1) / self.chunk_size;
    //info!("**total chunks: {}", &total_chunks);

    let session_id = Uuid::new_v4().to_string();

    for chunk_number in 0..total_chunks {
      //info!(">>lf chunk: {}", &chunk_number);
      let start = chunk_number * self.chunk_size;
      let end = std::cmp::min(start + self.chunk_size, total_bytes);
      let chunk = &file_bytes[start..end];

      let (final_chunk, content_encoding) = if self.compression_enabled {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(chunk).map_err(|e| AppError::CompressionError(e.to_string()))?;
        (encoder.finish().map_err(|e| AppError::CompressionError(e.to_string()))?, Some("deflate"))
      } else {
        (chunk.to_vec(), None)
      };

      //info!("size of lf chunk:{}", std::mem::size_of_val(&*final_chunk));
      let part = Part::bytes(final_chunk)
        .file_name(format!("{}_chunk_{}", &f_name, &chunk_number))
        //.mime_str(&f_type)
        .mime_str("application/octet-stream")
        .map_err(|e| AppError::ReqwestError(e.to_string()))?;
      
      let form = Form::new().
        text("session_id", session_id.clone()).
        text("total_chunks", total_chunks.to_string()).
        text("chunk_number", chunk_number.to_string()).  
        part("chunk", part);

      let mut req = self.client.post(&self.large_upload_url).multipart(form);
      if let Some(encoding) = content_encoding {
        req = req.header("content-encoding", encoding);
      }

      let timestamp = (js_sys::Date::now() / 1000.0) as u64;
      let signature = auth_signer.sign_with_key("/largeupload", timestamp).await?;

      req = req.header("x-timestamp", timestamp.to_string())
        .header("x-signature", signature);

      let resp = req.send().await.map_err(|e| AppError::UploadConnectionError(e.to_string()))?;

      if !resp.status().is_success() {
        // let status = resp.status();
        // let error_body = resp.text().await.map_err(|e| AppError::ReqwestError(e.to_string()))?;
        //error!("status code: {}, body: {}", status.as_str(), &error_body);
        //TODO: show different toast depending on errors
        document::eval(r#"
        var x = document.getElementById("upload-server-rl-toast");
        x.classList.add("show");
        setTimeout(function(){{x.classList.remove("show");}}, 2000);
        "#);
        break;
      } else {
        if chunk_number == total_chunks - 1 {
          let result = resp.json::<LargeUploadResponse>().await.map_err(|e| AppError::DeserializeError(e.to_string()))?;
          //info!("Processing complete for large file:\n{:?}", &result);
          assert_eq!(true, result.processed, "processing should be complete here!!");
          ob_results.set(result.orderbook_results);
          parse_results.set(result.parse_results);
        }
      }
    }
    Ok(())
  }

  pub async fn upload_small_file(&self,
    orders: Vec<FileUploadOrderType>,
    chunk_size: usize,
    auth_signer: AuthSignature,
    mut ob_results: Signal<Option<HashMap<String, FinalStats>>>
  ) -> Result<(), AppError> {
    let total_orders = orders.len();
    let total_chunks = if total_orders % chunk_size == 0 {total_orders/chunk_size} else { (total_orders / chunk_size) + 1 };

    let session_id = Uuid::new_v4().to_string();

    for (chunk_number, chunk) in orders.chunks(chunk_size).enumerate() {
      //info!(">>sf chunk: {}", &chunk_number);
      let upload_request = SmallUploadRequest {
        session_id: session_id.clone(),
        total_chunks,
        total_orders,
        chunk_number,
        orders: chunk.to_vec()
      };

      let mut buf = Vec::new();
      upload_request.serialize(&mut Serializer::new(&mut buf)).map_err(|e| AppError::SerializeError(e.to_string()))?;

      let (final_data, content_encoding) = if self.compression_enabled {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&buf).map_err(|e| AppError::CompressionError(e.to_string()))?;
        (encoder.finish().map_err(|e| AppError::CompressionError(e.to_string()))?, Some("deflate"))
      } else {
        (buf, None)
      };
      //info!("size of sf chunk:{}", std::mem::size_of_val(&*final_data));

      let mut req = self.client.post(&self.small_upload_url).body(final_data);
      if let Some(encoding) = content_encoding {
        req = req.header("content-encoding", encoding);
      }

      let timestamp = (js_sys::Date::now() / 1000.0) as u64;
      let signature = auth_signer.sign_with_key("/smallupload", timestamp).await?;

      req = req.header("x-timestamp", timestamp.to_string())
        .header("x-signature", signature);

      let resp = req.send().await.map_err(|e| AppError::UploadConnectionError(e.to_string()))?;

      if !resp.status().is_success() {
        // let status = resp.status();
        // let error_body = resp.text().await.map_err(|e| AppError::ReqwestError(e.to_string()))?;
        //TODO: show different toast depending on errors
        //error!("status code: {}, body: {}", status.as_str(), &error_body);
        document::eval(r#"
        var x = document.getElementById("upload-server-rl-toast");
        x.classList.add("show");
        setTimeout(function(){{x.classList.remove("show");}}, 2000);
        "#);
        break;
      } else {
        if chunk_number == total_chunks - 1 {
          let result = resp.json::<SmallUploadResponse>().await.map_err(|e| AppError::DeserializeError(e.to_string()))?;
          //info!("Processing complete for small file:\n{:?}", &result);
          assert_eq!(true, result.processed, "processing should be complete here!!");
          ob_results.set(result.orderbook_results);
        }
      }
    }
    Ok(())
  }
}