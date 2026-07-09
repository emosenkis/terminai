use std::sync::Arc;

use anyhow::Result;
use axum::{
  Router,
  extract::State,
  http::{Request, StatusCode, header},
  middleware::{self, Next},
  response::Response,
};
use rmcp::transport::streamable_http_server::{
  StreamableHttpServerConfig, StreamableHttpService,
  session::local::LocalSessionManager,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::tools::TerminaiMcpState;

pub struct McpServerHandle {
  pub url: String,
  pub port: u16,
  pub auth_token: String,
  cancellation: CancellationToken,
  _task: JoinHandle<()>,
}

impl Drop for McpServerHandle {
  fn drop(&mut self) {
    self.cancellation.cancel();
  }
}

pub async fn start_http_mcp_server(
  state: TerminaiMcpState,
  auth_token: String,
) -> Result<McpServerHandle> {
  let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
  let addr = listener.local_addr()?;
  let url = format!("http://{addr}/mcp");
  let cancellation = CancellationToken::new();

  let service: StreamableHttpService<TerminaiMcpState, LocalSessionManager> =
    StreamableHttpService::new(
      move || Ok(state.clone()),
      Default::default(),
      StreamableHttpServerConfig::default()
        .with_sse_keep_alive(None)
        .with_allowed_hosts([
          "localhost".to_string(),
          "127.0.0.1".to_string(),
          format!("127.0.0.1:{}", addr.port()),
        ])
        .with_cancellation_token(cancellation.child_token()),
    );

  let expected_token = Arc::<str>::from(auth_token.clone());
  let router = Router::new().nest_service("/mcp", service).route_layer(
    middleware::from_fn_with_state(expected_token, require_bearer_auth),
  );
  let server_cancellation = cancellation.clone();
  let task = tokio::spawn(async move {
    if let Err(err) = axum::serve(listener, router)
      .with_graceful_shutdown(async move {
        server_cancellation.cancelled_owned().await;
      })
      .await
    {
      log::error!("Terminai MCP server error: {err}");
    }
  });

  Ok(McpServerHandle {
    url,
    port: addr.port(),
    auth_token,
    cancellation,
    _task: task,
  })
}

async fn require_bearer_auth(
  State(expected_token): State<Arc<str>>,
  request: Request<axum::body::Body>,
  next: Next,
) -> Result<Response, StatusCode> {
  let Some(actual) = request
    .headers()
    .get(header::AUTHORIZATION)
    .and_then(|value| value.to_str().ok())
  else {
    return Err(StatusCode::UNAUTHORIZED);
  };

  if actual == format!("Bearer {expected_token}") {
    Ok(next.run(request).await)
  } else {
    Err(StatusCode::UNAUTHORIZED)
  }
}
