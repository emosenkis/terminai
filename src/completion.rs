use anyhow::{Context, Result, bail};
use minijinja::{Environment, UndefinedBehavior, context};

use crate::agent_launcher::AgentLaunchPlan;

const COMPLETION_PROMPT: &str = include_str!("../config/completion.jinja");

static COMPLETION_HTTP_CLIENT: std::sync::LazyLock<Result<reqwest::Client>> =
  std::sync::LazyLock::new(|| {
    // Custom credential headers must never follow redirects to another host.
    reqwest::Client::builder()
      .redirect(reqwest::redirect::Policy::none())
      .timeout(std::time::Duration::from_secs(30))
      .build()
      .context("failed to initialize completion HTTP client")
  });

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticPromptMarker {
  PromptStart,
  CommandStart,
  CommandExecuted,
  CommandFinished,
}

pub fn semantic_prompt_marker(escape: &str) -> Option<SemanticPromptMarker> {
  let rest = ["\x1b]133;", "\x1b]633;"]
    .iter()
    .find_map(|prefix| escape.strip_prefix(prefix))?;
  let marker = rest.chars().next()?;
  let rest = &rest[marker.len_utf8()..];
  if !(rest.starts_with([';', '\x07']) || rest.starts_with("\x1b\\")) {
    return None;
  }
  match marker {
    'A' => Some(SemanticPromptMarker::PromptStart),
    'B' => Some(SemanticPromptMarker::CommandStart),
    'C' => Some(SemanticPromptMarker::CommandExecuted),
    'D' => Some(SemanticPromptMarker::CommandFinished),
    _ => None,
  }
}

pub fn command_completion_prompt(terminal: &str, input: &str) -> String {
  let input_json = serde_json::to_string(input)
    .expect("serializing a string as JSON cannot fail");
  render_completion_prompt(
    context! { mode => "tracked-input", terminal, input_json },
  )
}

pub fn command_completion_suffix_prompt(terminal: &str) -> String {
  render_completion_prompt(context! { mode => "terminal-snapshot", terminal })
}

pub fn completion_with_prefix(input: &str, suggestion: &str) -> Option<String> {
  let suffix = suggestion
    .strip_prefix(input)
    .or_else(|| suggestion.trim_start().strip_prefix(input.trim_start()));
  let suffix = suffix.unwrap_or(suggestion);
  (!suffix.is_empty()).then(|| format!("{input}{suffix}"))
}

fn render_completion_prompt(context: minijinja::Value) -> String {
  let mut environment = Environment::new();
  environment.set_undefined_behavior(UndefinedBehavior::Strict);
  environment
    .render_str(COMPLETION_PROMPT, context)
    .expect("bundled completion prompt template must render")
}

pub async fn run_completion(plan: AgentLaunchPlan) -> Result<Vec<String>> {
  run_completion_with_field(plan, None).await
}

pub async fn run_completion_with_field(
  plan: AgentLaunchPlan,
  response_field: Option<&str>,
) -> Result<Vec<String>> {
  let mut command = tokio::process::Command::new(&plan.command);
  command
    .args(&plan.args)
    .envs(&plan.env)
    .current_dir(&plan.cwd)
    .kill_on_drop(true);
  let output =
    tokio::time::timeout(std::time::Duration::from_secs(30), command.output())
      .await
      .context("auto-completer invocation timed out")??;
  if !output.status.success() {
    bail!(
      "auto-completer invocation failed: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    );
  }
  completion_texts_with_field(
    &String::from_utf8(output.stdout)?,
    response_field,
  )
  .context("auto-completer returned no safe completion")
}

pub fn completion_texts(output: &str) -> Option<Vec<String>> {
  completion_texts_with_field(output, None)
}

pub fn completion_texts_with_field(
  output: &str,
  response_field: Option<&str>,
) -> Option<Vec<String>> {
  if let Some(field) = response_field {
    let value: serde_json::Value = serde_json::from_str(output).ok()?;
    return completion_values(value.get(field)?.clone());
  }
  let text = output.trim();
  let values = serde_json::from_str::<Vec<String>>(text)
    .unwrap_or_else(|_| vec![text.to_string()]);
  safe_completion_values(values)
}

fn completion_values(value: serde_json::Value) -> Option<Vec<String>> {
  let values = match value {
    serde_json::Value::String(text) => vec![text],
    serde_json::Value::Array(values) => values
      .into_iter()
      .map(|value| value.as_str().map(str::to_owned))
      .collect::<Option<Vec<_>>>()?,
    _ => return None,
  };
  safe_completion_values(values)
}

fn safe_completion_values(values: Vec<String>) -> Option<Vec<String>> {
  let mut safe = Vec::new();
  for value in values {
    if !value.trim().is_empty()
      && !value.chars().any(char::is_control)
      && !safe.contains(&value)
    {
      safe.push(value);
    }
    if safe.len() == 3 {
      break;
    }
  }
  (!safe.is_empty()).then_some(safe)
}

/// Run an instruction-trained model directly, without spawning a CLI.
pub async fn run_llm_completion(
  config: &crate::terminai_config::CompletionLlmConfig,
  response_field: Option<&str>,
  terminal: &str,
  input: Option<&str>,
) -> Result<Vec<String>> {
  use serde_json::json;

  let setting = |value: &Option<String>, name: &str| {
    value
      .clone()
      .or_else(|| std::env::var(name).ok())
      .filter(|value| !value.trim().is_empty())
      .with_context(|| {
        format!("set {name} or the corresponding auto-completion.llm setting")
      })
  };
  let url = setting(&config.url, "TERMINAI_COMPLETION_URL")?;
  let model = setting(&config.model, "TERMINAI_COMPLETION_MODEL")?;
  let field = response_field.unwrap_or("completions");
  let input_json = serde_json::to_string(input.unwrap_or_default())?;
  let response_field_json = serde_json::to_string(field)?;
  let prompt = render_completion_prompt(context! {
    mode => if input.is_some() { "tracked-input" } else { "terminal-snapshot" },
    terminal, input_json, response_field_json,
  });
  let response_format = json!({
    "type": "json_schema",
    "json_schema": {
      "name": "shell_completions", "strict": true,
      "schema": {
        "type": "object", "additionalProperties": false,
        "required": [field],
        "properties": { (field): {"type": "array", "items": {"type": "string"}} }
      }
    }
  });
  let mut environment = Environment::new();
  environment.set_undefined_behavior(UndefinedBehavior::Strict);
  environment.add_filter("json", |value: minijinja::Value| {
    serde_json::to_string(&value).expect("template values serialize to JSON")
  });
  environment.add_function("env", |name: String| {
    std::env::var(&name).map_err(|_| {
      minijinja::Error::new(
        minijinja::ErrorKind::InvalidOperation,
        format!("missing environment variable {name}"),
      )
    })
  });
  let data = context! {
    prompt, model,
    response_schema => response_format["json_schema"]["schema"].clone(),
    response_format => if config.structured_output { response_format.clone() } else { serde_json::Value::Null },
  };
  let url = environment
    .render_str(&url, &data)
    .context("failed to render completion URL")?;
  let body = if let Some(template) = &config.request_template {
    let rendered = environment
      .render_str(template, &data)
      .context("failed to render LLM request template")?;
    serde_json::from_str(&rendered)
      .context("LLM request template must render valid JSON")?
  } else {
    let mut body = json!({"model": model, "stream": false,
      "messages": [{"role": "user", "content": prompt}]});
    if config.structured_output {
      body["response_format"] = response_format;
    }
    body
  };
  let client = COMPLETION_HTTP_CLIENT
    .as_ref()
    .map_err(|error| anyhow::anyhow!("{error}"))?;
  let mut request = client.post(url).json(&body);
  let key_env = config
    .api_key_env
    .as_deref()
    .unwrap_or("TERMINAI_COMPLETION_API_KEY");
  match std::env::var(key_env) {
    Ok(key) => request = request.bearer_auth(key),
    Err(_) if config.api_key_env.is_some() => {
      bail!("missing credential environment variable {key_env}")
    }
    Err(_) => {}
  }
  for (header, template) in &config.headers {
    let value = environment.render_str(template, &data).with_context(|| {
      format!("failed to render completion header {header}")
    })?;
    let mut value = reqwest::header::HeaderValue::from_str(&value)
      .with_context(|| format!("invalid completion header {header}"))?;
    value.set_sensitive(true);
    let header = reqwest::header::HeaderName::from_bytes(header.as_bytes())
      .context("invalid completion HTTP header name")?;
    request = request.header(header, value);
  }
  let mut response = request
    .send()
    .await
    .map_err(|error| error.without_url())
    .context("completion HTTP request failed")?;
  if !response.status().is_success() {
    bail!(
      "completion HTTP request failed with status {}",
      response.status()
    );
  }
  let mut bytes = Vec::new();
  while let Some(chunk) = response
    .chunk()
    .await
    .map_err(|error| error.without_url())
    .context("failed to read completion HTTP response")?
  {
    if bytes.len() + chunk.len() > 1024 * 1024 {
      bail!("completion HTTP response exceeds 1 MiB");
    }
    bytes.extend_from_slice(&chunk);
  }
  let response: serde_json::Value = serde_json::from_slice(&bytes)
    .context("completion HTTP response must be JSON")?;
  let content = response
    .pointer(
      config
        .response_pointer
        .as_deref()
        .unwrap_or("/choices/0/message/content"),
    )
    .context(
      "completion HTTP response is missing the configured response pointer",
    )?;
  let suggestions = match content {
    serde_json::Value::String(text) => {
      completion_texts_with_field(text, Some(field))
    }
    value => value.get(field).cloned().and_then(completion_values),
  };
  suggestions
    .context("LLM returned no safe completion in the configured JSON field")
}

pub fn current_completion(
  current_generation: u64,
  result_generation: u64,
  result: std::result::Result<Vec<String>, String>,
) -> Option<Vec<String>> {
  (current_generation == result_generation)
    .then(|| result.ok())
    .flatten()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::agent_launcher::AgentLaunchMetadata;
  use std::collections::HashMap;

  #[test]
  fn extracts_only_the_configured_json_field() {
    let parse = |text| completion_texts_with_field(text, Some("answer"));
    assert_eq!(
      parse(r#"{"answer":" status","reason":"ignored"}"#),
      Some(vec![" status".into()])
    );
    assert_eq!(
      parse(r#"{"answer":["tatus","tatus","witch ","how","tash"]}"#),
      Some(vec!["tatus".into(), "witch ".into(), "how".into()])
    );
    for invalid in [
      r#"{"other":"oops"}"#,
      r#"{"answer":null}"#,
      r#"{"answer":["ok",123]}"#,
      r#"{"answer":"status\nwhoami"}"#,
      r#"{"answer":"\u001b[31m"}"#,
      "not JSON",
    ] {
      assert_eq!(parse(invalid), None);
    }
  }

  async fn mock_llm(
    response: serde_json::Value,
    status: axum::http::StatusCode,
  ) -> (
    String,
    tokio::task::JoinHandle<()>,
    tokio::sync::mpsc::Receiver<(axum::http::HeaderMap, serde_json::Value)>,
  ) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    let router = axum::Router::new().route(
      "/",
      axum::routing::post(
        move |headers: axum::http::HeaderMap,
              axum::Json(body): axum::Json<serde_json::Value>| {
          let tx = tx.clone();
          let response = response.clone();
          async move {
            let _ = tx.send((headers, body)).await;
            (status, axum::Json(response))
          }
        },
      ),
    );
    let task = tokio::spawn(async move {
      axum::serve(listener, router).await.unwrap();
    });
    (url, task, rx)
  }

  #[tokio::test]
  async fn direct_client_requests_schema_and_extracts_safe_suffixes() {
    use serde_json::json;
    let (url, server, mut requests) = mock_llm(
      json!({"choices": [{"message": {
        "content": r#"{"completions":[" status"," status","\n"," diff"]}"#
      }}]}),
      axum::http::StatusCode::OK,
    )
    .await;
    let config =
      serde_yaml::from_str(&format!("url: {url}\nmodel: local-instruct"))
        .unwrap();
    let result =
      run_llm_completion(&config, None, "$ git ", Some("git ")).await;
    server.abort();
    assert_eq!(result.unwrap(), vec![" status", " diff"]);
    let (_, body) = requests.recv().await.unwrap();
    assert_eq!(body["model"], "local-instruct");
    assert_eq!(body["stream"], false);
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
    assert_eq!(
      body["response_format"]["json_schema"]["schema"]["required"],
      json!(["completions"])
    );
    let prompt = body["messages"][0]["content"].as_str().unwrap();
    assert!(prompt.contains("JSON object"));
    assert!(prompt.contains("exactly the characters to append"));
  }

  #[tokio::test]
  async fn direct_client_supports_custom_provider_envelopes_and_requests() {
    use serde_json::json;
    let (url, server, mut requests) = mock_llm(
      json!({"output": {"answer": ["tatus"]}}),
      axum::http::StatusCode::OK,
    )
    .await;
    let config = serde_json::from_value(json!({
      "url": url, "model": "model\"name", "structured-output": false,
      "request-template": r#"{"model":{{ model|json }},"input":{{ prompt|json }},"format":{{ response_format|json }}}"#,
      "response-pointer": "/output"
    })).unwrap();
    let result = run_llm_completion(
      &config,
      Some("answer"),
      "untrusted \"text\"\n",
      Some("git s"),
    )
    .await;
    server.abort();
    assert_eq!(result.unwrap(), vec!["tatus"]);
    let (_, body) = requests.recv().await.unwrap();
    assert_eq!(body["model"], "model\"name");
    assert!(
      body["input"]
        .as_str()
        .unwrap()
        .contains("untrusted \"text\"\n")
    );
    assert!(body["format"].is_null());
  }

  #[tokio::test]
  async fn gemini_example_sends_templated_headers_and_native_schema() {
    use serde_json::json;
    let example = include_str!("../terminai.llm.example.yaml");
    let native = example
      .split("# auto-completion:\n")
      .nth(1)
      .unwrap()
      .split("#\n# For Vertex")
      .next()
      .unwrap()
      .lines()
      .map(|line| line.strip_prefix("# ").unwrap_or(line))
      .collect::<Vec<_>>()
      .join("\n");
    let mut config: crate::terminai_config::TerminaiConfig =
      serde_yaml::from_str(&format!("auto-completion:\n{native}")).unwrap();
    let (url, server, mut requests) = mock_llm(
      json!({"candidates": [{"content": {"parts": [{
        "text": r#"{"completions":["tatus"]}"#
      }]}}]}),
      axum::http::StatusCode::OK,
    )
    .await;
    let llm = config.auto_completion.llm.as_mut().unwrap();
    llm.url = Some(format!("{url}{{{{ '' }}}}"));
    llm.model = Some("test-model".into());
    llm.api_key_env = Some("PATH".into());
    llm
      .headers
      .insert("x-goog-api-key".into(), "{{ model }}".into());
    llm
      .headers
      .insert("x-test-env".into(), "{{ env('PATH') }}".into());
    let result = run_llm_completion(llm, None, "$ git s", Some("git s")).await;
    server.abort();
    assert_eq!(result.unwrap(), vec!["tatus"]);
    let (headers, body) = requests.recv().await.unwrap();
    assert_eq!(headers["x-goog-api-key"], "test-model");
    let path = std::env::var("PATH").unwrap();
    assert_eq!(headers["x-test-env"], path);
    assert_eq!(headers["authorization"], format!("Bearer {path}"));
    assert!(
      body["contents"][0]["parts"][0]["text"]
        .as_str()
        .unwrap()
        .contains("JSON object")
    );
    assert_eq!(
      body["generationConfig"]["responseFormat"]["text"]["schema"]["required"],
      json!(["completions"])
    );
  }

  #[tokio::test]
  async fn rejects_missing_template_credentials_before_sending() {
    use serde_json::json;
    let config = serde_json::from_value(json!({
      "url": "http://127.0.0.1:1", "model": "test",
      "headers": {"x-api-key": "{{ env('TERMINAI_TEST_MISSING_CREDENTIAL_7259') }}"}
    })).unwrap();
    let error = run_llm_completion(&config, None, "", None)
      .await
      .unwrap_err();
    assert!(
      error
        .to_string()
        .contains("failed to render completion header")
    );
  }

  #[tokio::test]
  async fn direct_client_rejects_http_errors_and_missing_fields() {
    use serde_json::json;
    for (status, body) in [
      (
        axum::http::StatusCode::UNAUTHORIZED,
        json!({"error":"secret body"}),
      ),
      (
        axum::http::StatusCode::OK,
        json!({"choices":[{"message":{"content":"oops"}}]}),
      ),
      (axum::http::StatusCode::OK, json!({"wrong":"envelope"})),
    ] {
      let (url, server, _) = mock_llm(body, status).await;
      let config =
        serde_json::from_value(json!({"url":url,"model":"test"})).unwrap();
      let error = run_llm_completion(&config, None, "", None)
        .await
        .unwrap_err();
      server.abort();
      assert!(!error.to_string().contains("secret body"));
    }
  }

  #[tokio::test]
  async fn direct_client_rejects_oversized_responses() {
    use serde_json::json;
    let (url, server, _) =
      mock_llm(json!("x".repeat(1024 * 1024)), axum::http::StatusCode::OK)
        .await;
    let config =
      serde_json::from_value(json!({"url":url,"model":"test"})).unwrap();
    let error = run_llm_completion(&config, None, "", None)
      .await
      .unwrap_err();
    server.abort();
    assert!(error.to_string().contains("exceeds 1 MiB"));
  }

  #[test]
  fn detects_common_semantic_prompt_markers() {
    use SemanticPromptMarker::*;
    assert_eq!(semantic_prompt_marker("\x1b]133;A\x07"), Some(PromptStart));
    assert_eq!(
      semantic_prompt_marker("\x1b]133;B\x1b\\"),
      Some(CommandStart)
    );
    assert_eq!(
      semantic_prompt_marker("\x1b]633;C\x07"),
      Some(CommandExecuted)
    );
    assert_eq!(
      semantic_prompt_marker("\x1b]633;D;0\x1b\\"),
      Some(CommandFinished)
    );
    assert_eq!(
      semantic_prompt_marker("\x1b]133;A;aid=42\x1b\\"),
      Some(PromptStart)
    );
    assert_eq!(semantic_prompt_marker("\x1b]133;AB\x07"), None);
    assert_eq!(semantic_prompt_marker("\x1b]133;"), None);
  }

  #[test]
  fn accepts_only_unique_non_executing_single_line_completions() {
    assert_eq!(
      completion_texts("[\"git status\",\"git diff\",\"git status\"]"),
      Some(vec!["git status".into(), "git diff".into()])
    );
    assert_eq!(
      completion_texts(" git status \n"),
      Some(vec!["git status".into()])
    );
    assert_eq!(
      completion_texts("[\" status\"]"),
      Some(vec![" status".into()])
    );
    assert_eq!(completion_texts("```sh\ngit status\n```"), None);
    assert_eq!(completion_texts("git status\rwhoami"), None);
    assert_eq!(completion_texts("\x1b[31mrm -rf /"), None);
    assert_eq!(completion_texts("   \n"), None);
  }

  #[test]
  fn prompt_requests_append_only_non_executing_shell_input() {
    let prompt = command_completion_prompt(
      "$ cargo test\nerror: failed\n$ git s",
      "git s",
    );
    assert!(prompt.contains("$ cargo test\nerror: failed"));
    assert!(prompt.contains("$ git s"));
    assert!(prompt.contains("<mode>tracked-input</mode>"));
    assert!(prompt.contains("<editable-input-json>\"git s\""));
    assert!(prompt.contains("exactly the characters to append"));
    assert!(prompt.contains("<example>"));
    assert!(!prompt.contains("<example "));

    let prompt = command_completion_suffix_prompt("$ git");
    assert!(prompt.contains("<mode>terminal-snapshot</mode>"));
    assert!(prompt.contains("<good>[\"tatus\",\"witch \"]</good>"));
  }

  #[test]
  fn suffix_results_tolerate_repeated_prefix_and_leading_whitespace() {
    assert_eq!(
      completion_with_prefix("git s", "tatus"),
      Some("git status".into())
    );
    assert_eq!(
      completion_with_prefix("git s", "git status"),
      Some("git status".into())
    );
    assert_eq!(
      completion_with_prefix("  git s", "git status"),
      Some("  git status".into())
    );
    assert_eq!(
      completion_with_prefix("git s", "   git status"),
      Some("git status".into())
    );
    assert_eq!(completion_with_prefix("git s", "git s"), None);
    assert_eq!(
      completion_with_prefix("", "git status"),
      Some("git status".into())
    );
  }

  #[tokio::test]
  async fn runs_the_configured_auto_completer() {
    #[cfg(windows)]
    let (command, args) =
      ("cmd.exe", vec!["/C".into(), "echo git status".into()]);
    #[cfg(not(windows))]
    let (command, args) = (
      "sh",
      vec!["-c".into(), "printf '[\"git status\"]\\n'".into()],
    );
    let plan = AgentLaunchPlan {
      command: command.into(),
      args,
      env: HashMap::new(),
      cwd: std::env::temp_dir(),
      metadata: AgentLaunchMetadata::default(),
    };

    assert_eq!(run_completion(plan).await.unwrap(), vec!["git status"]);
  }

  #[test]
  fn ignores_stale_or_disabled_results() {
    assert_eq!(
      current_completion(2, 2, Ok(vec!["git status".into()])),
      Some(vec!["git status".into()])
    );
    assert_eq!(current_completion(3, 2, Ok(vec!["stale".into()])), None);
    assert_eq!(current_completion(2, 2, Err("failed".into())), None);
  }
}
