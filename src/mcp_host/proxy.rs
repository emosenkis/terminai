use anyhow::{Context, Result, bail};
use rmcp::{
  RoleClient, RoleServer, ServerHandler, ServiceExt,
  model::{
    CallToolRequestParams, CallToolResult, ErrorData as McpError,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo,
  },
  service::Peer,
  transport::{
    IntoTransport, StreamableHttpClientTransport, stdio,
    streamable_http_client::StreamableHttpClientTransportConfig,
  },
};

pub async fn run_stdio_mcp_proxy() -> Result<()> {
  let auth_token = std::env::var("TERMINAI_MCP_AUTH_TOKEN")
    .context("TERMINAI_MCP_AUTH_TOKEN is required for terminai _mcp")?;
  let port = std::env::var("TERMINAI_MCP_PORT")
    .context("TERMINAI_MCP_PORT is required for terminai _mcp")?
    .parse::<u16>()
    .context("TERMINAI_MCP_PORT must be a valid TCP port")?;
  run_mcp_proxy_with_transport(port, auth_token, stdio()).await
}

async fn run_mcp_proxy_with_transport<T, E, A>(
  port: u16,
  auth_token: String,
  stdio_transport: T,
) -> Result<()>
where
  T: IntoTransport<RoleServer, E, A>,
  E: std::error::Error + Send + Sync + 'static,
{
  if auth_token.trim().is_empty() {
    bail!("TERMINAI_MCP_AUTH_TOKEN must not be empty");
  }

  let url = format!("http://127.0.0.1:{port}/mcp");
  let upstream_transport = StreamableHttpClientTransport::from_config(
    StreamableHttpClientTransportConfig::with_uri(url).auth_header(auth_token),
  );
  let upstream = ()
    .serve(upstream_transport)
    .await
    .context("failed to connect to Terminai HTTP MCP server")?;
  let proxy = TerminaiStdioMcpProxy {
    upstream: upstream.peer().clone(),
  };
  let server = proxy
    .serve(stdio_transport)
    .await
    .context("failed to start Terminai stdio MCP proxy")?;

  server.waiting().await?;
  drop(upstream);
  Ok(())
}

struct TerminaiStdioMcpProxy {
  upstream: Peer<RoleClient>,
}

impl TerminaiStdioMcpProxy {
  fn proxy_error(err: impl std::fmt::Display) -> McpError {
    McpError::internal_error(
      format!("Terminai MCP proxy request failed: {err}"),
      None,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::mcp_host::{TerminaiMcpState, start_http_mcp_server};
  use crate::shell::Shell;
  use tokio::sync::mpsc;

  #[tokio::test]
  async fn stdio_proxy_completes_handshake_and_forwards_tools() {
    let (shell, _rx) = Shell::spawn_command(
      "/bin/sh",
      &["-c".to_string(), "sleep 1".to_string()],
      24,
      80,
    )
    .expect("test shell should spawn");
    let (tx, _suggestion_rx) = mpsc::unbounded_channel();
    let state = TerminaiMcpState::new(shell.vt.clone(), tx);
    let http_server = start_http_mcp_server(state, "test-token".to_string())
      .await
      .expect("HTTP MCP server should start");

    let (client_transport, proxy_transport) = tokio::io::duplex(64 * 1024);
    let proxy_task = tokio::spawn(run_mcp_proxy_with_transport(
      http_server.port,
      "test-token".to_string(),
      proxy_transport,
    ));

    let client =
      ().serve(client_transport)
        .await
        .expect("client should initialize through stdio proxy");
    let tools = client
      .peer()
      .list_all_tools()
      .await
      .expect("stdio proxy should forward list_tools");

    assert!(tools.iter().any(|tool| tool.name == "read_terminal"));
    drop(client);
    proxy_task.abort();
  }
}

impl ServerHandler for TerminaiStdioMcpProxy {
  fn get_info(&self) -> ServerInfo {
    ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
  }

  async fn list_tools(
    &self,
    request: Option<PaginatedRequestParams>,
    _context: rmcp::service::RequestContext<rmcp::RoleServer>,
  ) -> Result<ListToolsResult, McpError> {
    self
      .upstream
      .list_tools(request)
      .await
      .map_err(Self::proxy_error)
  }

  async fn call_tool(
    &self,
    request: CallToolRequestParams,
    _context: rmcp::service::RequestContext<rmcp::RoleServer>,
  ) -> Result<CallToolResult, McpError> {
    self
      .upstream
      .call_tool(request)
      .await
      .map_err(Self::proxy_error)
  }
}
