use anyhow::Result;
use axum::Router;
use rmcp::transport::streamable_http_server::{
  StreamableHttpServerConfig, StreamableHttpService,
  session::local::LocalSessionManager,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::tools::TerminaiMcpState;

pub struct McpServerHandle {
  pub url: String,
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

  let router = Router::new().nest_service("/mcp", service);
  let server_cancellation = cancellation.clone();
  let task = tokio::spawn(async move {
    if let Err(err) = axum::serve(listener, router)
      .with_graceful_shutdown(async move {
        server_cancellation.cancelled_owned().await;
      })
      .await
    {
      log::error!("Termin.AI MCP server error: {err}");
    }
  });

  Ok(McpServerHandle {
    url,
    cancellation,
    _task: task,
  })
}
