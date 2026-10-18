//! pyJianYingDraft 示例对照合同。
//!
//! 上游每个示例都必须被登记、复制与对账：清单漏一项就失败，而不是静默少覆盖。
//! 覆盖三类来源：demo.py、tests/test_*.py、README 代码片段。

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(relative: &str) -> Value {
    let path = root().join(relative);
    serde_json::from_str(
        &fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} is missing or unreadable: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{} is invalid JSON: {error}", path.display()))
}

fn manifest() -> Value {
    read_json("examples/pyjyd/CASES.json")
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
fn draft_cases_have_a_reference_builder_and_are_compared() {
    let manifest = manifest();
    let parity = read_json("provenance/PYJYD_EXAMPLES_PARITY.json");
    let compared: BTreeSet<String> = parity["cases"]
        .as_array()
        .expect("parity cases")
        .iter()
        .filter(|case| case["status"] == json!("passed"))
        .map(|case| case["id"].as_str().unwrap().to_owned())
        .collect();

    let mut draft_cases = 0;
    for case in manifest["cases"].as_array().unwrap() {
        if case["category"] != json!("draft") {
            continue;
        }
        draft_cases += 1;
        let id = case["id"].as_str().unwrap();
        assert_eq!(
            case["status"],
            json!("compared"),
            "{id}: 草稿型用例必须已对照"
        );

        let case_path = root()
            .join("examples/pyjyd")
            .join(case["case"].as_str().expect("case path"));
        let spec: Value = serde_json::from_str(
            &fs::read_to_string(&case_path)
                .unwrap_or_else(|error| panic!("{id}: 缺少 case.json ({error})")),
        )
        .unwrap();

        let reference = root()
            .join("examples/pyjyd")
            .join(spec["reference"].as_str().expect("reference"));
        assert!(
            reference.is_file(),
            "{id}: 缺少参照构建器 {}",
            reference.display()
        );

        // plan 驱动或 post_build 驱动，二者必须择一
        let has_plan = spec.get("plan").is_some();
        let has_post = spec
            .get("post_build")
            .and_then(|value| value.as_array())
            .map(|commands| !commands.is_empty())
            .unwrap_or(false);
        assert!(has_plan || has_post, "{id}: 既没有 plan 也没有 post_build");
        if let Some(plan) = spec.get("plan").and_then(|value| value.as_str()) {
            let plan_path = root().join("examples/pyjyd").join(plan);
            let plan: Value =
                serde_json::from_str(&fs::read_to_string(&plan_path).unwrap()).unwrap();
            assert_eq!(
                plan["schema"],
                json!("jianying-cli-plan/v1"),
                "{id}: plan schema"
            );
        }

        // 每个已批准差异都必须写理由，不能只留一个标签
        if let Some(policy) = spec.get("policy") {
            let rationale = policy["rationale"].as_object().expect("policy.rationale");
            for name in ["approved_extra_kinds", "approved_missing_kinds"]
                .iter()
                .flat_map(|key| policy[*key].as_array().cloned().unwrap_or_default())
            {
                let name = name.as_str().unwrap();
                assert!(
                    rationale.contains_key(name),
                    "{id}: 已批准差异 {name} 没有写理由"
                );
            }
        }

        assert!(
            compared.contains(id),
            "{id}: provenance/PYJYD_EXAMPLES_PARITY.json 里没有通过的对照结论"
        );
    }
    assert_eq!(draft_cases, 6, "草稿型用例数量变了就要更新期望");
}

#[test]
fn behaviour_cases_are_covered_by_the_behaviour_artifact() {
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

#[test]
fn every_behaviour_test_is_executed_against_pyjianyingdraft() {
    let manifest = manifest();
    let artifact = read_json("provenance/PYJYD_BEHAVIOR_PARITY.json");
    assert_eq!(
        artifact["schema"],
        json!("jianying-pyjyd-behavior-parity/v1")
    );

    let executed: BTreeSet<String> = artifact["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .filter(|case| case["status"] == json!("passed"))
        .flat_map(|case| {
            case["covers"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
        })
        .collect();

    let mut declared: BTreeSet<String> = BTreeSet::new();
    for case in manifest["cases"].as_array().unwrap() {
        if case["category"] != json!("behavior") {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        assert_eq!(
            case["status"],
            json!("compared"),
            "{id}: 行为型用例必须已执行对照"
        );
        assert_eq!(
            case["artifact"],
            json!("provenance/PYJYD_BEHAVIOR_PARITY.json"),
            "{id}: 必须指向行为对照产物"
        );
        let tests: Vec<String> = case["upstream_tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap().to_owned())
            .collect();
        assert!(!tests.is_empty(), "{id}: 行为用例必须对应上游测试");
        for name in &tests {
            assert!(
                executed.contains(name),
                "{name}: 未被任何行为用例实际执行覆盖"
            );
        }
        declared.extend(tests);
    }
    assert_eq!(declared, executed, "清单与产物的行为测试集必须一致");
    assert_eq!(declared.len(), 34, "行为测试数变了就要更新期望");
}

#[test]
fn every_readme_snippet_is_classified() {
    let manifest = manifest();
    let snippets = read_json("examples/pyjyd/README_SNIPPETS.json");
    let reference = manifest["readme_snippets"]
        .as_str()
        .expect("manifest 指向 README 片段索引");
    assert!(root().join(reference).is_file());

    let entries = snippets["snippets"].as_array().expect("snippets");
    assert_eq!(
        entries.len() as u64,
        snippets["total"].as_u64().unwrap(),
        "total 与逐条清点不一致"
    );
    assert!(entries.len() >= 32, "上游 README 片段被漏掉");

    // 每段都要有分类；out_of_scope 还必须有理由
    let reasons = snippets["out_of_scope_reasons"]
        .as_object()
        .expect("out_of_scope_reasons");
    for entry in entries {
        let mapped = entry["maps_to"].as_str().expect("maps_to");
        assert_ne!(
            mapped, "unmapped",
            "{}#{} 未分类",
            entry["file"], entry["index"]
        );
        if let Some(key) = mapped.strip_prefix("out_of_scope:") {
            assert!(reasons.contains_key(key), "out_of_scope {key} 缺少理由");
        }
        assert_eq!(
            entry["sha256"].as_str().map(str::len),
            Some(64),
            "{}#{} 缺少 sha256",
            entry["file"],
            entry["index"]
        );
    }
}
