use dotenvy::dotenv;

fn main() {
   // Tell Cargo that if the env file changes, to rerun this build script.
  println!("cargo::rerun-if-changed=.env");

  dotenv().expect("failed to load .env file");

  if let Ok(key) = std::env::var("HMAC_KEY") {
    println!("cargo::rustc-env=HMAC_KEY={}", key);
  } else {
    panic!("HMAC key must be set at compile time!");
  }

  if let Ok(key) = std::env::var("HEALTH_CHECK_URL") {
    println!("cargo::rustc-env=HEALTH_CHECK_URL={}", key);
  } else {
    panic!("HEALTH_CHECK_URL must be set at compile time!");
  }
  if let Ok(key) = std::env::var("SMALL_UPLOAD_URL") {
    println!("cargo::rustc-env=SMALL_UPLOAD_URL={}", key);
  } else {
    panic!("SMALL_UPLOAD_URL must be set at compile time!");
  }
  if let Ok(key) = std::env::var("LARGE_UPLOAD_URL") {
    println!("cargo::rustc-env=LARGE_UPLOAD_URL={}", key);
  } else {
    panic!("LARGE_UPLOAD_URL must be set at compile time!");
  }
  if let Ok(key) = std::env::var("WSS_URL") {
    println!("cargo::rustc-env=WSS_URL={}", key);
  } else {
    panic!("WSS_URL must be set at compile time!");
  }

}