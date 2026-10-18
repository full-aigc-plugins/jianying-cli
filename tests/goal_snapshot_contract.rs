//! 目标快照合同：快照必须是"跑出来的"，且结构完整、自检全绿。

use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn snapshot() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("provenance/GOAL_SNAPSHOT.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("GOAL_SNAPSHOT.json exists"))
        .expect("snapshot is valid JSON")
}

#[test]
fn snapshot_records_a_green_selftest_and_real_counts() {
    let report = snapshot();
    assert_eq!(report["schema"], json!("jianying-goal-snapshot/v1"));
    assert!(
        report["generated_at"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "快照必须带生成时间"
    );
    assert!(
        report["git"]["commit"]
            .as_str()
            .map(|c| c.len() >= 7)
            .unwrap_or(false),
        "快照必须记录 commit"
    );

    let selftest = &report["selftest"];
    assert_eq!(selftest["ok"], json!(true), "快照里的自检必须是绿的");
    let summary = &selftest["summary"];
    assert_eq!(summary["fail"], json!(0));
    let total = summary["total"].as_i64().unwrap();
    assert!(total >= 7, "自检项太少: {total}");
    assert_eq!(
        summary["pass"].as_i64().unwrap() + summary["skip"].as_i64().unwrap(),
        total
    );
    let ids: Vec<&str> = selftest["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| check["id"].as_str().unwrap())
        .collect();
    for expected in ["capabilities", "skills", "mcp", "plan", "draft"] {
        assert!(ids.contains(&expected), "缺少检查项 {expected}: {ids:?}");
    }

    let surface = &report["surface"];
    assert!(surface["commands"].as_i64().unwrap() >= 80);
    assert!(surface["capabilities"].as_i64().unwrap() >= 150);
    assert!(surface["skills"].as_i64().unwrap() >= 10);

    // 覆盖摘要来自生成物，不能是 0
    assert!(report["coverage"]["pyjyd"]["classes"].as_i64().unwrap() >= 60);
    assert!(
        report["coverage"]["editor_skill"]["methods"]
            .as_i64()
            .unwrap()
            >= 10
    );
    let claims = report["claims"].as_object().expect("claims");
    assert!(claims.len() >= 5, "声明项应覆盖六条验收线");
}
