use std::{net::SocketAddr, time::{SystemTime, UNIX_EPOCH}};
use axum::{extract::{ConnectInfo, Request}, middleware::Next, response::IntoResponse};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use tokio::sync::OnceCell;

use crate::{get_origin, midwares::app_state::RequestContext, EXPECTED_ORIGIN};
use super::app_state::AppError;

static HMAC_KEY: OnceCell<String> = OnceCell::const_new();

async fn get_hmac_key() -> String {
  std::env::var("HMAC_KEY").expect("HMAC_KEY should be available!")
}

async fn hmac_sha256(data: &str) -> String {
  type HmacSha256 = Hmac<Sha256>;

  let hmac_key = HMAC_KEY.get_or_init(get_hmac_key).await;

  let mut mac = HmacSha256::new_from_slice(hmac_key.as_bytes()).expect("HMAC can take key of any size");
  mac.update(data.as_bytes());
  let result = mac.finalize();
  hex::encode(result.into_bytes())
}

pub async fn ip_tracker_with_auth(
  ConnectInfo(addr): ConnectInfo<SocketAddr>,
  mut req: Request,
  next: Next
) -> Result<impl IntoResponse, AppError> {

  // println!("[IPtracker MW] got headers");
  // for (k, v) in req.headers().iter() {
  //   println!("{:?}: {:?}", k, v);
  // }
  let expected_client_origin = EXPECTED_ORIGIN.get_or_init(get_origin).await;

  let headers = req.headers();
  let uri_path = req.uri().path();
  let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_err(|e| AppError::InternalError(e.to_string()))?
      .as_secs();
  println!("[**]uri path: {}", uri_path);

  let origin_str = headers
    .get("origin")
    .and_then(|v| v.to_str().ok())
    .unwrap_or("NA");

  if !origin_str.contains(expected_client_origin) {
    return Err(AppError::Unauthorized("Invalid origin".to_string()));
  };

  let origin = origin_str.to_string();

  let (timestamp, signature) = if uri_path == "/wslob" {
    // for Websocket protocols
    headers.get("sec-websocket-protocol")
      .and_then(|v| v.to_str().ok())
      .and_then(|proto| {
        let mut parts = proto.split(',').map(|s| s.trim());
        let ws_ts = parts.next().and_then(|s| s.parse::<u64>().ok());
        let ws_sig = parts.next();
        Some((ws_ts, ws_sig))
      })
      .unwrap_or((None, None))
  } else {
    // for HTTP headers
    let http_ts = headers.get("x-timestamp")
      .and_then(|v| v.to_str().ok())
      .and_then(|v| v.parse::<u64>().ok());
    let http_sig = headers.get("x-signature")
      .and_then(|v| v.to_str().ok());
    (http_ts, http_sig)
  };


  if let (Some(ts), Some(sig)) = (timestamp, signature) {

    if now - ts > 60 {
      return Err(AppError::Unauthorized("Request expired".to_string()))
    }

    let expected = hmac_sha256(&format!("{}{}", uri_path, ts)).await;
    if sig != expected {
      return Err(AppError::Unauthorized("Invalid signature".to_string()));
    }
  } else {
    return Err(AppError::Unauthorized("Missing timestamp or signature".to_string()));
  }

  // Get IP from headers, or fallback to socket address
  let remote_ip = req.headers()
    .get("x-forwarded-for")
    .and_then(|h| h.to_str().ok())
    .unwrap_or(&addr.ip().to_string())
    .to_string();
  //println!("[**]remote_ip: {}", &remote_ip);

  let user_agent = req.headers()
    .get("user-agent")
    .and_then(|h| h.to_str().ok())
    .unwrap_or("NA")
    .to_string();  
  //println!("[**]user_agent: {}", &user_agent);
  let timestamp_string = timestamp.expect("timestamp should exist here").to_string();
  let signature_string = signature.expect("signature should exist here").to_string();

  req.extensions_mut().insert(RequestContext {
    remote_ip,
    origin,
    user_agent,
    timestamp: timestamp_string,
    signature: signature_string
  });

  Ok(next.run(req).await)
}