//! MCP 命令桥合同：stdio 会话内可枚举命令、执行任意 CLI 命令、批量执行并拒绝越权路径。

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "jianying-mcp-bridge-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// 启动一次 stdio 会话，写入全部请求后读取全部响应。
fn session(requests: &[Value]) -> Vec<Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jianying"));
    command
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for request in requests {
            writeln!(stdin, "{request}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn initialize() -> Value {
    json!({
        "jsonrpc":"2.0","id":1,"method":"initialize",
        "params":{"protocolVersion":"2024-11-05","capabilities":{},
                  "clientInfo":{"name":"contract","version":"1"}}
    })
}

fn call(id: u64, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
           "params":{"name":name,"arguments":arguments}})
}

fn tool_data(response: &Value) -> Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("missing tool content: {response}"));
    let parsed: Value = serde_json::from_str(text).unwrap();
    parsed.get("data").cloned().unwrap_or(parsed)
}

fn plan_file(root: &Path) -> PathBuf {
    let path = root.join("plan.json");
    std::fs::write(
        &path,
        serde_json::to_string(&json!({
            "schema":"jianying-cli-plan/v1","name":"mcp-bridge",
            "canvas":{"width":320,"height":180,"fps":30},
            "tracks":[{"type":"text","segments":[
                {"start_us":0,"duration_us":1000000,"text":"MCP"}
            ]}]
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

#[test]
fn stdio_session_exposes_the_command_bridge_and_runs_any_command() {
    let root = temp_root("run");
    let plan = plan_file(&root);
    let draft = root.join("draft");

    let responses = session(&[
        initialize(),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        call(
            3,
            "jianying_command_run",
            json!({"path":"project build","args":[plan.to_string_lossy(), "--out", draft.to_string_lossy()]}),
        ),
        call(
            4,
            "jianying_command_batch",
            json!({"steps":[{"path":"eval","args":[draft.to_string_lossy()]}]}),
        ),
    ]);

    let tools = responses
        .iter()
        .find(|response| response["id"] == json!(2))
        .expect("tools/list response")["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    for expected in [
        "jianying_command_list",
        "jianying_command_run",
        "jianying_command_batch",
    ] {
        assert!(
            tools.contains(&expected.to_owned()),
            "缺少工具 {expected}: {tools:?}"
        );
    }

    let run = responses
        .iter()
        .find(|response| response["id"] == json!(3))
        .expect("command_run response");
    let run_data = tool_data(run);
    assert_eq!(run_data["ok"], json!(true), "{run_data}");
    assert_eq!(run_data["exit_code"], json!(0));
    assert_eq!(run_data["response"]["ok"], json!(true));
    assert!(draft.join("draft_content.json").is_file());

    let batch = responses
        .iter()
        .find(|response| response["id"] == json!(4))
        .expect("command_batch response");
    let batch_data = tool_data(batch);
    assert_eq!(batch_data["ok"], json!(true));
    assert_eq!(batch_data["succeeded"], json!(1));
    assert_eq!(batch_data["steps"][0]["ok"], json!(true));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn command_run_refuses_paths_outside_the_catalog_and_nested_mcp() {
    let responses = session(&[
        initialize(),
        call(
            2,
            "jianying_command_run",
            json!({"path":"rm -rf /","args":[]}),
        ),
        call(
            3,
            "jianying_command_run",
            json!({"path":"mcp serve","args":[]}),
        ),
    ]);

    for id in [2, 3] {
        let response = responses
            .iter()
            .find(|response| response["id"] == json!(id))
            .expect("error response");
        assert_eq!(
            response["result"]["isError"],
            json!(true),
            "越权路径必须报错: {response}"
        );
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(
            text.contains("unknown command path") || text.contains("refusing to nest"),
            "错误信息必须可操作: {text}"
        );
    }
}

#[test]
fn command_list_matches_the_cli_catalog() {
    let responses = session(&[initialize(), call(2, "jianying_command_list", json!({}))]);
    let data = tool_data(
        responses
            .iter()
            .find(|response| response["id"] == json!(2))
            .expect("command_list response"),
    );
    assert_eq!(data["schema"], json!("jianying-command-catalog/v1"));
    let total = data["total"].as_i64().unwrap();
    assert!(total >= 40, "命令目录过小: {total}");
    assert_eq!(data["commands"].as_array().unwrap().len() as i64, total);
}

#[test]
fn inspect_tools_answer_read_only_questions_about_a_draft() {
    let root = temp_root("inspect");
    let plan = plan_file(&root);
    let draft = root.join("draft");
    // 先用命令桥建出草稿，再用检视工具读它
    session(&[
        initialize(),
        call(
            2,
            "jianying_command_run",
            json!({"path":"project build","args":[plan.to_string_lossy(), "--out", draft.to_string_lossy()]}),
        ),
    ]);

    let responses = session(&[
        initialize(),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        call(
            3,
            "jianying_draft_inspect",
            json!({"draft": draft.to_string_lossy()}),
        ),
        call(
            4,
            "jianying_timeline_segments",
            json!({"draft": draft.to_string_lossy(), "track":"text"}),
        ),
        call(
            5,
            "jianying_eval",
            json!({"draft": draft.to_string_lossy()}),
        ),
    ]);

    let tools: Vec<String> = responses
        .iter()
        .find(|response| response["id"] == json!(2))
        .unwrap()["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect();
    for expected in [
        "jianying_draft_inspect",
        "jianying_timeline_segments",
        "jianying_media_probe",
        "jianying_eval",
    ] {
        assert!(tools.contains(&expected.to_owned()), "缺少工具 {expected}");
    }

    let inspect = tool_data(
        responses
            .iter()
            .find(|response| response["id"] == json!(3))
            .unwrap(),
    );
    assert_eq!(inspect["name"], json!("mcp-bridge"));
    assert_eq!(inspect["duration_us"], json!(1_000_000));

    let segments = tool_data(
        responses
            .iter()
            .find(|response| response["id"] == json!(4))
            .unwrap(),
    );
    assert_eq!(segments.as_array().unwrap().len(), 1);
    assert_eq!(segments[0]["type"], json!("text"));
    assert_eq!(segments[0]["duration_us"], json!(1_000_000));

    let score = tool_data(
        responses
            .iter()
            .find(|response| response["id"] == json!(5))
            .unwrap(),
    );
    assert!(score["score"].as_i64().unwrap() >= 0);
    assert!(score["issues"].is_array());
    let _ = std::fs::remove_dir_all(root);
}
