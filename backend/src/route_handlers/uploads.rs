use axum::{body::Bytes, extract::{Multipart, State}, http::HeaderMap, Extension, Json};
use serde::Deserialize;

use crate::{
  file_upload::{parser::parse_file_orders, processor::{decompress_if_needed, process_uploaded_orders, LargeUploadResponse, LargeUploadSessionManager, SmallUploadRequest, SmallUploadResponse, SmallUploadSessionManager}},
  midwares::app_state::{estimate_orders_from_1stchunk, AppError, PostgresDBPool, RateLimiter, RequestContext}
};

pub async fn small_upload_handler(
  State(state): State<SmallUploadSessionManager>,
  Extension(rate_limiter): Extension<RateLimiter>,
  Extension(postgres): Extension<PostgresDBPool>,
  Extension(req_ctx): Extension<RequestContext>,
  headers: HeaderMap,
  body: Bytes
) -> Result<Json<SmallUploadResponse>, AppError> {
  // decompress if required
  let content_encoding = headers.get("content-encoding").and_then(|v| v.to_str().ok());
  //println!("content encoding for sf handler: {:?}", &content_encoding);
  let decompressed_data = decompress_if_needed(&body, content_encoding)?;
  // deserialize the payload
  let payload = match <SmallUploadRequest>::deserialize(&mut rmp_serde::Deserializer::new(&decompressed_data[..])) {
    Ok(de_payload) => de_payload,
    Err(e) => {
      println!("couldnot deserialize the small file upload request: {:?}", e);
      return Err(AppError::DeserializeError(e.to_string()))
    }
  };

  // destructure the payload
  let SmallUploadRequest { session_id, total_chunks, total_orders, chunk_number, orders } = payload;
  
  let remote_ip = req_ctx.remote_ip;
  let origin = req_ctx.origin;
  let user_agent = req_ctx.user_agent;

  // check 1st chunk and return early if ratelimit exceeds
  if chunk_number == 0 {
    if let Err(e) = rate_limiter.would_exceed_limit(&remote_ip, &total_orders).await {
      // log the ratelimited visit in db 
      postgres.record_in_db(&remote_ip, &origin, &user_agent, 0, true);
      return Err(e);
    };
  }

  state.store_chunk(&session_id, chunk_number, orders, total_chunks).await;

  let is_complete = state.is_upload_complete(&session_id).await;

  if is_complete {
    // get complete order vector
    let complete_orders = state.get_all_chunks(&session_id).await.map_err(|e| AppError::InternalError(e))?;

    // log in db
    postgres.record_in_db(&remote_ip, &origin, &user_agent, total_orders, false);
    // log in redis
    rate_limiter.record_orders(&remote_ip, total_orders).await?;

    let ob_results = process_uploaded_orders(complete_orders);

    // Clean up the chunks after processing
    state.clear_chunks(&session_id).await.map_err(|e| AppError::InternalError(e))?;

    return Ok(Json(SmallUploadResponse {
      orderbook_results: Some(ob_results),
      processed: true
    }));
  }

  // For intermediate chunks, just return acknowledgment
  Ok(Json(
    SmallUploadResponse {
      orderbook_results: None,
      processed: false
    }))
}

pub async fn large_upload_handler (
  State(state): State<LargeUploadSessionManager>,
  Extension(rate_limiter): Extension<RateLimiter>,
  Extension(postgres): Extension<PostgresDBPool>,
  Extension(req_ctx): Extension<RequestContext>,
  headers: HeaderMap,
  mut multipart: Multipart
) -> Result<Json<LargeUploadResponse>, AppError> {

  let content_encoding = headers.get("content-encoding").and_then(|v| v.to_str().ok());
  //println!("content encoding for lf handler: {:?}", &content_encoding);

  // Extract multipart fields
  let mut session_id = None;
  let mut total_chunks = None;
  let mut chunk_number = None;
  let mut chunk_data = None;

  while let Some(field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
    match field.name() {
      Some("session_id") => session_id = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?),
      Some("total_chunks") => total_chunks = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?.parse::<usize>().map_err(|_| AppError::BadRequest("Invalid total_chunks value".to_string()))?),
      Some("chunk_number") => chunk_number = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?.parse::<usize>().map_err(|_| AppError::BadRequest("Invalid chunk_number value".to_string()))?),
      Some("chunk") => chunk_data = Some(field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?),
      _ => {}
    }
  }

  // Validate we got all required fields
  let session_id = session_id.ok_or(AppError::BadRequest("Missing session_id".to_string()))?;
  let total_chunks = total_chunks.ok_or(AppError::BadRequest("Missing total_chunks".to_string()))?;
  let chunk_number = chunk_number.ok_or(AppError::BadRequest("Missing chunk_number".to_string()))?;
  let chunk_data = chunk_data.ok_or(AppError::BadRequest("Missing chunk_data".to_string()))?;

  // decompress if needed
  let decompressed_chunk: Bytes = decompress_if_needed(&chunk_data, content_encoding)?.into();

  let remote_ip = req_ctx.remote_ip;
  let origin = req_ctx.origin;
  let user_agent = req_ctx.user_agent;
  // estimate total orders with 1st chunk and return early if ratelimit exceeds
  if chunk_number == 0 {
    let estimated_orders = estimate_orders_from_1stchunk(&decompressed_chunk, &total_chunks);
    if let Err(e) = rate_limiter.would_exceed_limit(&remote_ip, &estimated_orders).await {
      // log the ratelimited visit in db 
      postgres.record_in_db(&remote_ip, &origin, &user_agent, 0, true);
      return Err(e);
    };
  }
  
  state.store_chunk(&session_id, chunk_number, decompressed_chunk, total_chunks).await;

  let is_complete = state.is_upload_complete(&session_id).await;

  if is_complete {
    // get complete file data
    let complete_data = state.get_all_chunks(&session_id).await.map_err(|e| AppError::InternalError(e))?;

    let (parsed_orders, duration, raw_cnt, invalid_cnt) = parse_file_orders(&complete_data);
    let total_orders = parsed_orders.len();
    // return early if no valid orders were found
    if total_orders == 0 {
      return Ok(Json(
        LargeUploadResponse {
          orderbook_results: None,
          parse_results: Some((duration, raw_cnt, invalid_cnt)),
          processed: true
        }
      ));
    }
    // now we check for ratelimits with actual orders
    rate_limiter.would_exceed_limit(&remote_ip, &total_orders).await?;
    // log in db
    postgres.record_in_db(&remote_ip, &origin, &user_agent, total_orders, false);
    // log in redis
    rate_limiter.record_orders(&remote_ip, total_orders).await?;

    let ob_results = process_uploaded_orders(parsed_orders);

    // Clean up the chunks after processing
    state.clear_chunks(&session_id).await.map_err(|e| AppError::InternalError(e))?;

    return Ok(Json(
      LargeUploadResponse {
        orderbook_results: Some(ob_results),
        parse_results: Some((duration, raw_cnt, invalid_cnt)),
        processed: true
      }
    ));
  }

  // For intermediate chunks, just return acknowledgment
  Ok(Json(
    LargeUploadResponse {
      orderbook_results: None,
      parse_results: None,
      processed: false
    }))
}