use crate::{capabilities, error_contract, job_runner, mcp::ToolRegistry, store};
use anyhow::{anyhow, bail, Result};
use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use jianying_cli_contract::SuccessEnvelope;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    },
    service::RequestContext,
    transport::{
        stdio,
        streamable_http_server::{
            session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
        },
    },
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use serde_json::{Map, Value};
use std::future::Future;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Streamable HTTP 成功响应的编码偏好。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpResponseMode {
    /// 简单请求优先返回 JSON；需要流式消息时自动回退到 SSE。
    JsonPreferred,
    /// 所有成功请求均使用 `text/event-stream` 响应。
    Sse,
}

/// MCP Streamable HTTP 服务监听与安全配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpServeOptions {
    /// TCP 监听地址。
    pub bind: SocketAddr,
    /// 单一 MCP endpoint，例如 `/mcp`。
    pub endpoint: String,
    /// 可选 Bearer token；非回环监听时必填。
    pub bearer_token: Option<String>,
    /// 允许的 Host 或 `host:port` 列表。
    pub allowed_hosts: Vec<String>,
    /// 允许的浏览器 Origin 列表。
    pub allowed_origins: Vec<String>,
    /// JSON/SSE 响应偏好。
    pub response_mode: HttpResponseMode,
}

impl HttpServeOptions {
    /// 在绑定端口前验证远程访问的最小安全边界。
    pub fn validate(&self) -> Result<()> {
        if !self.endpoint.starts_with('/')
            || self.endpoint.contains('?')
            || self.endpoint.contains('#')
        {
            bail!("MCP endpoint must be an absolute path without query or fragment");
        }
        if self
            .bearer_token
            .as_ref()
            .is_some_and(|token| token.trim().is_empty())
        {
            bail!("MCP Bearer token must not be empty");
        }
        if !self.bind.ip().is_loopback() {
            if self.bearer_token.is_none() {
                bail!("non-loopback MCP bind requires a Bearer token via --token-env");
            }
            if self.allowed_hosts.is_empty() {
                bail!("non-loopback MCP bind requires at least one --allowed-host");
            }
        }
        if self.allowed_hosts.iter().any(|host| host.trim().is_empty()) {
            bail!("MCP allowed hosts must not contain empty values");
        }
        if self
            .allowed_origins
            .iter()
            .any(|origin| origin.trim().is_empty())
        {
            bail!("MCP allowed origins must not contain empty values");
        }
        Ok(())
    }
}

/// 由官方 Rust MCP SDK 承载的剪映服务。
#[derive(Debug, Clone)]
struct JianyingMcpServer {
    state_root: PathBuf,
}

impl JianyingMcpServer {
    fn new(state_root: PathBuf) -> Self {
        Self { state_root }
    }

    fn tool_result(result: Result<Value>) -> CallToolResult {
        match result {
            Ok(data) => CallToolResult::structured(
                serde_json::to_value(SuccessEnvelope::new(data)).unwrap_or(Value::Null),
            ),
            Err(error) => CallToolResult::structured_error(
                serde_json::to_value(error_contract::error_envelope(&error)).unwrap_or(Value::Null),
            ),
        }
    }

    fn call(&self, name: &str, arguments: Map<String, Value>) -> Result<Value> {
        let arguments = Value::Object(arguments);
        match name {
            "jianying_capabilities" => capabilities::manifest(),
            "jianying_doctor" => store::doctor(),
            "jianying_job_run" => {
                let job = required_path(&arguments, "job")?;
                let output = optional_path(&arguments, "out")?;
                job_runner::run_persisted(&job, output.as_deref(), &self.state_root)
            }
            "jianying_job_retry" => {
                let task_id = required_string(&arguments, "task_id")?;
                job_runner::retry_persisted(task_id, &self.state_root)
            }
            "jianying_job_list" => job_runner::list_persisted(&self.state_root),
            "jianying_job_show" => {
                let task_id = required_string(&arguments, "task_id")?;
                job_runner::show_persisted(task_id, &self.state_root)
            }
            "jianying_job_cancel" => {
                let task_id = required_string(&arguments, "task_id")?;
                job_runner::cancel_persisted(task_id, &self.state_root)
            }
            "jianying_job_audit" => {
                let task_id = required_string(&arguments, "task_id")?;
                job_runner::audit_persisted(task_id, &self.state_root)
            }
            "jianying_render_frame" => {
                let draft = required_path(&arguments, "draft")?;
                let out = required_path(&arguments, "out")?;
                let at_us = arguments
                    .get("at_us")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| anyhow!("at_us is required"))?;
                let scale = arguments
                    .get("scale")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.5);
                crate::render::render_frame(&draft, &out, at_us, scale)
            }
            "jianying_draft_inspect" => {
                let draft = required_path(&arguments, "draft")?;
                crate::draft::inspect(&draft)
            }
            "jianying_timeline_segments" => {
                let draft = required_path(&arguments, "draft")?;
                let track = arguments.get("track").and_then(Value::as_str);
                crate::timeline_ops::segments(&draft, track)
            }
            "jianying_media_probe" => {
                let path = required_path(&arguments, "path")?;
                let info = crate::probe::probe(&path)?;
                serde_json::to_value(info).map_err(Into::into)
            }
            "jianying_eval" => {
                let draft = required_path(&arguments, "draft")?;
                let deep = arguments
                    .get("deep")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let policy = match arguments.get("policy").and_then(Value::as_str) {
                    Some(path) => crate::eval_policy::Policy::load(Path::new(path))?,
                    None => crate::eval_policy::Policy::default(),
                };
                if deep {
                    crate::eval_ops::evaluate_deep_with(&draft, &policy)
                } else {
                    crate::eval_ops::evaluate_with(&draft, &policy)
                }
            }
            "jianying_command_list" => command_list(),
            "jianying_command_run" => command_run(&arguments),
            "jianying_command_batch" => command_batch(&arguments),
            _ => Err(anyhow!("unknown tool {name}")),
        }
    }
}

impl ServerHandler for JianyingMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "jianying-cli",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("Execute versioned JianYing jobs through the shared Rust handler.")
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(rmcp_tools())))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        ToolRegistry::standard()
            .get(name)
            .and_then(|descriptor| serde_json::to_value(descriptor).ok())
            .and_then(|value| serde_json::from_value(value).ok())
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if self.get_tool(&request.name).is_none() {
            return Err(McpError::invalid_params(
                format!("unknown tool {}", request.name),
                None,
            ));
        }
        let arguments = request.arguments.unwrap_or_default();
        Ok(Self::tool_result(self.call(&request.name, arguments)).into())
    }
}

/// 返回 MCP SDK `tools/list` 与 CLI `mcp tools` 共用的确定性目录。
pub fn tools() -> Result<Value> {
    Ok(serde_json::to_value(ToolRegistry::standard().list())?)
}

/// 使用官方 `rmcp` SDK 的 stdio transport 运行 MCP 会话。
pub fn serve(state_root: &Path) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        JianyingMcpServer::new(state_root.to_path_buf())
            .serve(stdio())
            .await?
            .waiting()
            .await?;
        Result::<()>::Ok(())
    })
}

/// 使用官方 `rmcp` Streamable HTTP transport 运行远程 MCP 服务。
pub fn serve_http(state_root: &Path, options: HttpServeOptions) -> Result<()> {
    options.validate()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let cancellation = CancellationToken::new();
        let mut config = StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(options.response_mode == HttpResponseMode::JsonPreferred)
            .with_cancellation_token(cancellation.child_token());
        if !options.allowed_hosts.is_empty() {
            config = config.with_allowed_hosts(options.allowed_hosts.clone());
        }
        if !options.allowed_origins.is_empty() {
            config = config.with_allowed_origins(options.allowed_origins.clone());
        }

        let service_root = state_root.to_path_buf();
        let service: StreamableHttpService<JianyingMcpServer, LocalSessionManager> =
            StreamableHttpService::new(
                move || Ok(JianyingMcpServer::new(service_root.clone())),
                LocalSessionManager::default().into(),
                config,
            );
        let mut router = Router::new().nest_service(&options.endpoint, service);
        if let Some(token) = options.bearer_token {
            router = router.layer(middleware::from_fn_with_state(Arc::new(token), bearer_auth));
        }

        let listener = tokio::net::TcpListener::bind(options.bind).await?;
        eprintln!(
            "MCP {:?} listening on http://{}{}",
            options.response_mode, options.bind, options.endpoint
        );
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = tokio::signal::ctrl_c().await;
                cancellation.cancel();
            })
            .await?;
        Result::<()>::Ok(())
    })
}

async fn bearer_auth(
    State(expected): State<Arc<String>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|actual| constant_time_eq(actual.as_bytes(), expected.as_bytes()));
    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
        )
            .into_response();
    }
    next.run(request).await
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left_byte, right_byte)| {
            difference | (left_byte ^ right_byte)
        })
        == 0
}

fn rmcp_tools() -> Vec<Tool> {
    ToolRegistry::standard()
        .list()
        .into_iter()
        .filter_map(|descriptor| serde_json::to_value(descriptor).ok())
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect()
}

/// 当前 CLI 可执行文件；`command_*` 工具通过它复用同一套 handler。
fn cli_binary() -> Result<PathBuf> {
    std::env::current_exe().map_err(|error| anyhow!("cannot resolve current executable: {error}"))
}

/// 机器可读命令目录里的全部路径，进程内缓存一次。
fn catalog_paths() -> Result<&'static std::collections::BTreeSet<String>> {
    static PATHS: std::sync::OnceLock<std::collections::BTreeSet<String>> =
        std::sync::OnceLock::new();
    if let Some(paths) = PATHS.get() {
        return Ok(paths);
    }
    let output = std::process::Command::new(cli_binary()?)
        .args(["commands", "--json"])
        .output()
        .map_err(|error| anyhow!("cannot run `commands --json`: {error}"))?;
    if !output.status.success() {
        bail!("`commands --json` failed with {}", output.status);
    }
    let envelope: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| anyhow!("`commands --json` is not JSON: {error}"))?;
    let paths = envelope["data"]["commands"]
        .as_array()
        .ok_or_else(|| anyhow!("command catalog is missing data.commands"))?
        .iter()
        .filter_map(|entry| entry["path"].as_str().map(str::to_owned))
        .collect();
    Ok(PATHS.get_or_init(|| paths))
}

fn command_list() -> Result<Value> {
    let output = std::process::Command::new(cli_binary()?)
        .args(["commands", "--json"])
        .output()
        .map_err(|error| anyhow!("cannot run `commands --json`: {error}"))?;
    if !output.status.success() {
        bail!("`commands --json` failed with {}", output.status);
    }
    let envelope: Value = serde_json::from_slice(&output.stdout)?;
    Ok(envelope["data"].clone())
}

/// 执行一条 CLI 命令。只接受命令目录里存在的路径，避免把 MCP 变成任意进程通道。
fn command_run(arguments: &Value) -> Result<Value> {
    let path = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("path is required"))?;
    let extra: Vec<String> = arguments
        .get("args")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    run_one(path, &extra)
}

fn run_one(path: &str, extra: &[String]) -> Result<Value> {
    let paths = catalog_paths()?;
    if !paths.contains(path) {
        bail!("unknown command path {path:?}; call jianying_command_list to discover paths");
    }
    if path == "mcp serve" || path.starts_with("mcp ") {
        bail!("refusing to nest an MCP server inside command_run");
    }
    let mut command = std::process::Command::new(cli_binary()?);
    command.args(path.split_whitespace());
    command.args(extra);
    if !extra.iter().any(|arg| arg == "--json") {
        command.arg("--json");
    }
    let output = command
        .output()
        .map_err(|error| anyhow!("cannot run {path}: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
    Ok(serde_json::json!({
        "ok": output.status.success(),
        "path": path,
        "args": extra,
        "exit_code": output.status.code(),
        "response": response,
        "stderr": String::from_utf8_lossy(&output.stderr).trim(),
    }))
}

fn command_batch(arguments: &Value) -> Result<Value> {
    let steps = arguments
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("steps is required"))?;
    if steps.is_empty() {
        bail!("steps must not be empty");
    }
    let stop_on_error = arguments
        .get("stop_on_error")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mut results = Vec::new();
    let mut succeeded = 0_u64;
    let mut failed = 0_u64;
    for (index, step) in steps.iter().enumerate() {
        let path = step
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("steps[{index}].path is required"))?;
        let extra: Vec<String> = step
            .get("args")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let result = run_one(path, &extra)?;
        let ok = result["ok"].as_bool().unwrap_or(false);
        if ok {
            succeeded += 1;
        } else {
            failed += 1;
        }
        results.push(result);
        if !ok && stop_on_error {
            break;
        }
    }
    Ok(serde_json::json!({
        "ok": failed == 0,
        "total": steps.len(),
        "succeeded": succeeded,
        "failed": failed,
        "steps": results,
    }))
}

fn required_path(arguments: &Value, field: &str) -> Result<PathBuf> {
    optional_path(arguments, field)?.ok_or_else(|| anyhow!("arguments.{field} is required"))
}

fn optional_path(arguments: &Value, field: &str) -> Result<Option<PathBuf>> {
    match arguments.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(path)) => Ok(Some(PathBuf::from(path))),
        Some(_) => Err(anyhow!("arguments.{field} must be a string")),
    }
}

fn required_string<'a>(arguments: &'a Value, field: &str) -> Result<&'a str> {
    arguments[field]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("arguments.{field} is required"))
}
