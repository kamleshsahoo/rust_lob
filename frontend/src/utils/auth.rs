use js_sys::{wasm_bindgen::JsValue, Array, Uint8Array};
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, CryptoKey, SubtleCrypto};

use super::server::AppError;
const SECRET_KEY: &str = env!("HMAC_KEY");

pub struct AuthSignature {
  subtle: SubtleCrypto,
  algo: js_sys::Object,
  crypto_key: CryptoKey
}

impl AuthSignature {
  pub async fn new() -> Result<Self, AppError> {
    
    let crypto = window().expect("global window should exist!").crypto().map_err(|e| AppError::WasmError(format!("{:?}", e)))?;
    let subtle = crypto.subtle();

    let algo = js_sys::Object::new();
    js_sys::Reflect::set(&algo, &JsValue::from_str("name"), &JsValue::from_str("HMAC")).map_err(|e| AppError::WasmError(format!("{:?}", e)))?;
    js_sys::Reflect::set(&algo, &JsValue::from_str("hash"), &JsValue::from_str("SHA-256")).map_err(|e| AppError::WasmError(format!("{:?}",e)))?;

    //init and store the key
    let crypto_key = Self::init_crypto_key(&subtle, &algo).await?;
    
    Ok(Self { subtle, algo, crypto_key })
  }

  async fn init_crypto_key(subtle: &SubtleCrypto, algo: &js_sys::Object) -> Result<CryptoKey, AppError> {
  
    let key_bytes = SECRET_KEY.as_bytes();
    let key_array = Uint8Array::new_with_length(key_bytes.len() as u32);
    key_array.copy_from(key_bytes);
    
    let usages = Array::new();
    usages.push(&JsValue::from_str("sign"));
  
    let key_promise = subtle.import_key_with_object(
      "raw",
      &key_array,
      algo,
      false,
      &usages
    ).map_err(|e| AppError::WasmError(format!("{:?}", e)))?;
  
    let c_key: CryptoKey = JsFuture::from(key_promise).await.map_err(|e| AppError::WasmError(format!("{:?}", e)))?.into();

    Ok(c_key)
  }

  pub async fn sign_with_key(&self, path: &str, timestamp: u64) -> Result<String, AppError> {
    let message = format!("{}{}", path, timestamp);
    let message_bytes = message.as_bytes();

    let message_array = Uint8Array::new_with_length(message_bytes.len() as u32);
    message_array.copy_from(message_bytes);

    //sign promise
    let sign_promise = self.subtle.sign_with_object_and_buffer_source(&self.algo, &self.crypto_key, &message_array).map_err(|e| AppError::WasmError(format!("{:?}", e)))?;

    let signature_buffer = JsFuture::from(sign_promise).await.map_err(|e| AppError::WasmError(format!("{:?}", e)))?;
    let signature_array = Uint8Array::new(&signature_buffer);

    // Convert to hex string
    let mut result = String::with_capacity(signature_array.length() as usize * 2);
    for i in 0..signature_array.length() {
      let byte = signature_array.get_index(i);
      result.push_str(&format!("{:02x}", byte));
    }

    Ok(result)
  }
}