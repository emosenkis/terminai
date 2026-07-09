use anyhow::{Context, Result, bail};
use rmcp::{
  RoleClient, ServerHandler, ServiceExt,
  model::{
    CallToolRequestParams, CallToolResult, ErrorData as McpError,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo,
  },
  service::Peer,
  transport::{
    StreamableHttpClientTransport, stdio,
    streamable_http_client::StreamableHttpClientTransportConfig,
  },
};

pub async fn run_stdio_mcp_proxy(port: u16) -> Result<()> {
  let auth_token = std::env::var("TERMINAI_MCP_AUTH_TOKEN")
    .context("TERMINAI_MCP_AUTH_TOKEN is required for terminai _mcp")?;
  if auth_token.trim().is_empty() {
    bail!("TERMINAI_MCP_AUTH_TOKEN must not be empty");
  }

  let url = format!("http://127.0.0.1:{port}/mcp");
  let transport = StreamableHttpClientTransport::from_config(
    StreamableHttpClientTransportConfig::with_uri(url).auth_header(auth_token),
  );
  let upstream = ()
    .serve(transport)
    .await
    .context("failed to connect to Terminai HTTP MCP server")?;
  let proxy = TerminaiStdioMcpProxy {
    upstream: upstream.peer().clone(),
  };
  let server = proxy
    .serve(stdio())
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
