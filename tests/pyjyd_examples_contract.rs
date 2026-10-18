//! pyJianYingDraft 示例对照合同。
//!
//! 上游每个示例都必须被登记、复制与对账：清单漏一项就失败，而不是静默少覆盖。

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manifest() -> Value {
    let path = root().join("examples/pyjyd/CASES.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("CASES.json exists"))
        .expect("CASES.json is valid JSON")
}

fn upstream_test_functions(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).expect("upstream example is readable");
    text.lines()
        .filter_map(|line| line.strip_prefix("def "))
        .filter_map(|rest| rest.split('(').next())
        .filter(|name| name.starts_with("test_"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_upstream_example_is_copied_and_registered() {
    let manifest = manifest();
    assert_eq!(manifest["schema"], json!("jianying-pyjyd-examples/v1"));

    let upstream = manifest["upstream"]["commit"]
        .as_str()
        .expect("upstream commit recorded");
    assert_eq!(upstream.len(), 40, "上游 commit 必须是完整 sha");

    let cases = manifest["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 11, "示例数量变了就要更新清单");

    let mut seen_sources = BTreeSet::new();
    let mut declared_tests: Vec<String> = Vec::new();
    for case in cases {
        let source = case["source"].as_str().expect("source");
        assert!(
            seen_sources.insert(source.to_owned()),
            "duplicate case source {source}"
        );
        let copied = root().join("examples/pyjyd/upstream").join(source);
        assert!(copied.is_file(), "上游示例未复制: {source}");

        // 清单里登记的测试函数必须与复制过来的文件逐条一致
        let on_disk = upstream_test_functions(&copied);
        let declared: Vec<String> = case["upstream_tests"]
            .as_array()
            .expect("upstream_tests")
            .iter()
            .map(|name| name.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(declared, on_disk, "{source} 的测试清单与上游不一致");
        declared_tests.extend(declared);
    }
    assert_eq!(
        declared_tests.len(),
        manifest["total_tests"].as_u64().unwrap() as usize,
        "total_tests 与逐条清点不一致"
    );
    assert!(declared_tests.len() >= 51, "上游测试函数被漏掉");
}

#[test]
fn draft_cases_have_a_reference_builder_and_a_plan() {
    let manifest = manifest();
    for case in manifest["cases"].as_array().unwrap() {
        if case["category"] != json!("draft") {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let status = case["status"].as_str().expect("status");
        if status == "pending" {
            // 待补的用例必须写清楚为什么还没对比，不允许默默缺席
            assert!(
                case["pending_reason"]
                    .as_str()
                    .map(|reason| !reason.is_empty())
                    .unwrap_or(false),
                "{id}: pending 用例必须写 pending_reason"
            );
            continue;
        }
        assert_eq!(status, "compared", "{id}: 未知 status");
        let ours = &case["ours"];
        let plan = root()
            .join("examples/pyjyd")
            .join(ours["plan"].as_str().unwrap());
        let runner = root()
            .join("examples/pyjyd")
            .join(ours["runner"].as_str().unwrap());
        assert!(plan.is_file(), "{id}: 缺少等价 plan {}", plan.display());
        assert!(
            runner.is_file(),
            "{id}: 缺少参照构建器 {}",
            runner.display()
        );

        let plan: Value = serde_json::from_str(&fs::read_to_string(&plan).unwrap()).unwrap();
        assert_eq!(
            plan["schema"],
            json!("jianying-cli-plan/v1"),
            "{id}: plan schema"
        );

        // 每个草稿型用例都要有明确的比对策略（已批准差异必须写理由）
        let case_path = root()
            .join("examples/pyjyd/cases")
            .join(id.trim_start_matches("test_").replace('_', "-"))
            .join("case.json");
        if case_path.is_file() {
            let case_json: Value =
                serde_json::from_str(&fs::read_to_string(&case_path).unwrap()).unwrap();
            if let Some(policy) = case_json.get("policy") {
                let rationale = policy["rationale"].as_object().expect("policy.rationale");
                for kind in ["approved_extra_kinds", "approved_missing_kinds"]
                    .iter()
                    .flat_map(|key| policy[*key].as_array().cloned().unwrap_or_default())
                {
                    let name = kind.as_str().unwrap();
                    assert!(
                        rationale.contains_key(name),
                        "{id}: 已批准差异 {name} 没有写理由"
                    );
                }
            }
        }
    }
}

#[test]
fn behaviour_cases_point_at_real_evidence() {
    let manifest = manifest();
    for case in manifest["cases"].as_array().unwrap() {
        if case["category"] != json!("behavior") {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let evidence = case["evidence"].as_array().expect("evidence");
        assert!(!evidence.is_empty(), "{id}: 行为型用例必须给出证据文件");
        for path in evidence {
            let path = root().join(path.as_str().unwrap());
            assert!(path.exists(), "{id}: 证据文件不存在 {}", path.display());
        }
        assert!(
            case["compare"]
                .as_str()
                .map(|text| !text.is_empty())
                .unwrap_or(false),
            "{id}: 必须说明比对口径"
        );
    }
}
