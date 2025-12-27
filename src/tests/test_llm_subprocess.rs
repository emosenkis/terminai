// Integration test for LLM subprocess management

use termin::llm_subprocess::{LlmSubprocess, LlmSubprocessConfig};

#[tokio::test]
#[ignore] // Only run when explicitly requested (requires Python/uv)
async fn test_subprocess_lifecycle() {
  // Initialize logging for test visibility
  let _ = env_logger::builder()
    .is_test(true)
    .filter_level(log::LevelFilter::Debug)
    .try_init();

  let config = LlmSubprocessConfig::default();

  println!("Spawning subprocess...");
  let subprocess = LlmSubprocess::spawn(config)
    .await
    .expect("Failed to spawn subprocess");

  println!("Subprocess spawned on port {}", subprocess.port());

  // Verify subprocess is running
  assert!(
    subprocess.is_running().await,
    "Subprocess should be running"
  );

  // Verify we got a port
  assert!(subprocess.port() > 0, "Port should be set");

  // Verify we got a secret
  assert!(!subprocess.secret().is_empty(), "Secret should be set");

  // Verify base URL is correct
  assert!(
    subprocess.base_url().starts_with("http://"),
    "Base URL should start with http://"
  );

  // Test basic HTTP connectivity
  let client = reqwest::Client::new();
  let response = client
    .get(format!("{}/health", subprocess.base_url()))
    .send()
    .await
    .expect("Failed to connect to subprocess");

  assert!(
    response.status().is_success(),
    "Health check should succeed"
  );

  let body: serde_json::Value =
    response.json().await.expect("Failed to parse JSON");
  assert_eq!(body["status"], "healthy");

  println!("Shutting down subprocess...");
  subprocess
    .shutdown()
    .await
    .expect("Failed to shutdown subprocess");

  println!("Test complete");
}

#[tokio::test]
#[ignore] // Only run when explicitly requested
async fn test_subprocess_authentication() {
  let _ = env_logger::builder()
    .is_test(true)
    .filter_level(log::LevelFilter::Debug)
    .try_init();

  let config = LlmSubprocessConfig::default();
  let subprocess = LlmSubprocess::spawn(config)
    .await
    .expect("Failed to spawn subprocess");

  let client = reqwest::Client::new();

  // Test without secret - should fail
  let response = client
    .get(format!("{}/", subprocess.base_url()))
    .send()
    .await
    .expect("Failed to connect");

  assert_eq!(response.status(), 401, "Should reject without secret");

  // Test with correct secret - should succeed
  let response = client
    .get(format!("{}/", subprocess.base_url()))
    .header("x-ag-ui-secret", subprocess.secret())
    .send()
    .await
    .expect("Failed to connect");

  assert!(
    response.status().is_success(),
    "Should accept with correct secret"
  );

  // Test with incorrect secret - should fail
  let response = client
    .get(format!("{}/", subprocess.base_url()))
    .header("x-ag-ui-secret", "wrong-secret")
    .send()
    .await
    .expect("Failed to connect");

  assert_eq!(
    response.status(),
    401,
    "Should reject with incorrect secret"
  );

  subprocess.shutdown().await.expect("Failed to shutdown");
}
