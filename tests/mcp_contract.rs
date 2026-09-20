use jianying_jobs::{JobRecord, JobState, SqliteJobStore};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
}

fn temp_root() -> PathBuf {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let root = std::env::temp_dir().join(format!(
        "jyc-mcp-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

fn stdio_exchange(state_root: &Path, requests: &[Value]) -> Vec<Value> {
    let mut child = bin()
        .args(["mcp", "serve", "--state-root", state_root.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    for request in requests {
        writeln!(child.stdin.as_mut().unwrap(), "{request}").unwrap();
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn reserve_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn wait_for_listener(port: u16) {
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("MCP HTTP listener did not start on port {port}");
}

fn initialize_http(port: u16, host: &str, origin: Option<&str>, token: Option<&str>) -> String {
    let body = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-06-18","capabilities":{},
        "clientInfo":{"name":"http-test","version":"1"}
    }})
    .to_string();
    let mut headers = format!(
        "POST /mcp HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(origin) = origin {
        headers.push_str(&format!("Origin: {origin}\r\n"));
    }
    if let Some(token) = token {
        headers.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    headers.push_str("\r\n");
    headers.push_str(&body);

    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream.write_all(headers.as_bytes()).unwrap();
    let mut response = String::new();
    match stream.read_to_string(&mut response) {
        Ok(_) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) => {}
        Err(error) => panic!("cannot read MCP HTTP response: {error}"),
    }
    response
}

#[test]
fn mcp_tools_catalog_has_machine_schemas() {
    let output = bin().args(["mcp", "tools", "--json"]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tools = stdout_json(&output)["data"].as_array().unwrap().clone();
    let job_run = tools
        .iter()
        .find(|tool| tool["name"] == "jianying_job_run")
        .unwrap();
    assert_eq!(job_run["inputSchema"]["required"], json!(["job"]));
    assert_eq!(job_run["annotations"]["readOnlyHint"], false);
    assert!(job_run["outputSchema"].is_object());
}

#[test]
fn http_transports_are_discoverable_and_remote_bind_fails_closed_without_token() {
    let help = bin().args(["mcp", "serve", "--help"]).output().unwrap();
    assert!(help.status.success());
    let help_text = String::from_utf8(help.stdout).unwrap();
    assert!(help_text.contains("--transport"));
    assert!(help_text.contains("streamable-http"));
    assert!(help_text.contains("sse"));
    assert!(help_text.contains("--allowed-host"));
    assert!(help_text.contains("--allowed-origin"));
    assert!(help_text.contains("--token-env"));

    let denied = bin()
        .args([
            "mcp",
            "serve",
            "--transport",
            "streamable-http",
            "--bind",
            "0.0.0.0:8765",
            "--allowed-host",
            "192.0.2.10:8765",
            "--token-env",
            "JIANYING_TEST_MISSING_TOKEN",
        ])
        .env_remove("JIANYING_TEST_MISSING_TOKEN")
        .output()
        .unwrap();
    assert_eq!(denied.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&denied.stderr).contains("Bearer token"),
        "{}",
        String::from_utf8_lossy(&denied.stderr)
    );

    let denied_host = bin()
        .args([
            "mcp",
            "serve",
            "--transport",
            "sse",
            "--bind",
            "0.0.0.0:8765",
            "--token-env",
            "JIANYING_TEST_MCP_TOKEN",
        ])
        .env("JIANYING_TEST_MCP_TOKEN", "test-secret")
        .output()
        .unwrap();
    assert_eq!(denied_host.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&denied_host.stderr).contains("--allowed-host"),
        "{}",
        String::from_utf8_lossy(&denied_host.stderr)
    );

    let capabilities = bin().args(["capabilities", "--json"]).output().unwrap();
    assert!(capabilities.status.success());
    let manifest = stdout_json(&capabilities);
    for id in [
        "mcp.stdio",
        "mcp.streamable_http",
        "mcp.sse_response",
        "mcp.job_lifecycle",
    ] {
        let capability = manifest["data"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("missing capability {id}"));
        assert_eq!(capability["status"], "supported");
    }
}

#[test]
fn streamable_http_and_sse_profiles_enforce_auth_host_and_origin() {
    for (transport, expected_content_type) in [
        ("streamable-http", "application/json"),
        ("sse", "text/event-stream"),
    ] {
        let port = reserve_port();
        let state_root = temp_root().join(transport);
        let host = format!("127.0.0.1:{port}");
        let mut child = bin()
            .args([
                "mcp",
                "serve",
                "--transport",
                transport,
                "--bind",
                &host,
                "--state-root",
                state_root.to_str().unwrap(),
                "--allowed-host",
                &host,
                "--allowed-origin",
                "http://pad.local",
                "--token-env",
                "JIANYING_TEST_MCP_TOKEN",
            ])
            .env("JIANYING_TEST_MCP_TOKEN", "test-secret")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait_for_listener(port);

        let unauthorized = initialize_http(port, &host, None, None);
        assert!(unauthorized.starts_with("HTTP/1.1 401"), "{unauthorized}");

        let bad_host = initialize_http(port, "evil.invalid", None, Some("test-secret"));
        assert!(bad_host.starts_with("HTTP/1.1 403"), "{bad_host}");

        let bad_origin = initialize_http(
            port,
            &host,
            Some("http://evil.invalid"),
            Some("test-secret"),
        );
        assert!(bad_origin.starts_with("HTTP/1.1 403"), "{bad_origin}");

        let accepted = initialize_http(port, &host, Some("http://pad.local"), Some("test-secret"));
        assert!(accepted.starts_with("HTTP/1.1 200"), "{accepted}");
        assert!(
            accepted
                .to_ascii_lowercase()
                .contains(&format!("content-type: {expected_content_type}")),
            "{accepted}"
        );
        assert!(accepted.contains("jianying-cli"), "{accepted}");

        child.kill().unwrap();
        child.wait().unwrap();
    }
}

#[test]
fn stdio_mcp_and_cli_share_job_handler_and_error_type() {
    let root = temp_root();
    let job_path = root.join("invalid-job.json");
    write_json(
        &job_path,
        &json!({"schema":"jianying-job/v99","operation":"inspect","project":{
            "type":"existing","source":"draft"
        }}),
    );
    let cli = bin()
        .args(["job", "run", job_path.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(1));
    let cli_error = stdout_json(&cli)["error"].clone();

    let state_root = root.join("mcp-state");
    let mut child = bin()
        .args(["mcp", "serve", "--state-root", state_root.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}
        }}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
            "name":"jianying_job_run","arguments":{"job":job_path}
        }}),
    ];
    for request in requests {
        writeln!(child.stdin.as_mut().unwrap(), "{request}").unwrap();
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "jianying-cli");
    let served_tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert!(served_tools.len() >= 3);
    let served_job = served_tools
        .iter()
        .find(|tool| tool["name"] == "jianying_job_run")
        .unwrap();
    assert_eq!(served_job["inputSchema"]["required"], json!(["job"]));
    assert_eq!(served_job["annotations"]["readOnlyHint"], false);
    for name in [
        "jianying_job_list",
        "jianying_job_show",
        "jianying_job_cancel",
        "jianying_job_audit",
    ] {
        assert!(
            served_tools.iter().any(|tool| tool["name"] == name),
            "missing {name}"
        );
    }
    assert_eq!(responses[2]["result"]["isError"], true);
    assert_eq!(
        responses[2]["result"]["structuredContent"]["error"]["type"],
        cli_error["type"]
    );
    assert_eq!(
        responses[2]["result"]["structuredContent"]["error"]["details"]["actual"],
        cli_error["details"]["actual"]
    );
}

#[test]
fn stdio_mcp_exposes_persistent_job_lifecycle() {
    let root = temp_root();
    let state_root = root.join("mcp-state");
    let store = SqliteJobStore::new(state_root.join("jobs.sqlite3"));
    let task_id = "task-mcp-cancel";
    store
        .save(&JobRecord::new(task_id.to_owned(), root.join("job.json"), None).unwrap())
        .unwrap();

    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}
    }});
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}});
    let cancelled = stdio_exchange(
        &state_root,
        &[
            initialize.clone(),
            initialized.clone(),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
                "name":"jianying_job_cancel","arguments":{"task_id":task_id}
            }}),
        ],
    );
    assert_eq!(cancelled.len(), 2);
    assert_eq!(
        cancelled[1]["result"]["structuredContent"]["data"]["state"],
        "cancelled"
    );
    assert_eq!(store.load(task_id).unwrap().state, JobState::Cancelled);

    let observed = stdio_exchange(
        &state_root,
        &[
            initialize,
            initialized,
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
                "name":"jianying_job_list","arguments":{}
            }}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{
                "name":"jianying_job_show","arguments":{"task_id":task_id}
            }}),
            json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{
                "name":"jianying_job_audit","arguments":{"task_id":task_id}
            }}),
        ],
    );
    assert_eq!(observed.len(), 4);
    let tool_results = &observed[1..];
    assert!(tool_results.iter().any(|response| {
        response["result"]["structuredContent"]["data"]
            .as_array()
            .is_some_and(|records| records.iter().any(|record| record["task_id"] == task_id))
    }));
    assert!(tool_results.iter().any(|response| {
        response["result"]["structuredContent"]["data"]["task_id"] == task_id
            && response["result"]["structuredContent"]["data"]["state"] == "cancelled"
    }));
    assert!(tool_results.iter().any(|response| {
        response["result"]["structuredContent"]["data"]["task_id"] == task_id
            && response["result"]["structuredContent"]["data"]["history"]
                .as_array()
                .is_some_and(|history| history.len() == 2)
    }));
}
