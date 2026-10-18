//! pyJianYingDraft API 覆盖报告合同。
//!
//! 报告由 `tools/pyjyd_api_coverage.py` 从上游 ast 清单 + `PYJYD_API_MAPPING.json`
//! 生成。CI 没有上游检出，因此这里只校验**已提交**的报告自洽：
//! 类级 uncovered 必须为 0，partial 必须写明缺口方法，out_of_scope 必须写明原因。

use serde_json::Value;

fn report() -> Value {
    serde_json::from_str(include_str!("../provenance/PYJYD_API_COVERAGE.json")).unwrap()
}

#[test]
fn no_pyjyd_class_is_left_uncovered() {
    let report = report();
    assert_eq!(report["schema"], "jianying-pyjyd-api-coverage/v1");
    let uncovered = report["summary"]["uncovered_classes"].as_array().unwrap();
    assert!(
        uncovered.is_empty(),
        "类级未覆盖必须为 0，实际: {uncovered:?}"
    );
    assert_eq!(report["upstream"]["commit"].as_str().unwrap().len(), 40);
}

#[test]
fn every_class_records_its_mapping_and_gaps() {
    let report = report();
    let classes = report["classes"].as_array().unwrap();
    assert_eq!(
        report["summary"]["classes"],
        serde_json::json!(classes.len())
    );
    assert!(classes.len() >= 60, "上游类数过少: {}", classes.len());

    let mut covered = 0;
    let mut partial = 0;
    let mut out_of_scope = 0;
    for class in classes {
        let key = class["class"].as_str().unwrap();
        let status = class["status"].as_str().unwrap();
        match status {
            "covered" => {
                covered += 1;
                assert!(
                    class["methods"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|method| method["status"] != "uncovered"),
                    "{key} 是 covered 但有 uncovered 方法"
                );
            }
            "partial" => {
                partial += 1;
                assert!(
                    !class["ours"].as_str().unwrap().is_empty(),
                    "{key} partial 必须写 ours"
                );
                let missing = class["methods"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|method| method["status"] == "uncovered")
                    .count();
                assert!(missing > 0, "{key} partial 必须列出至少一个缺口方法");
            }
            "out_of_scope" => {
                out_of_scope += 1;
                assert!(
                    !class["note"].as_str().unwrap().is_empty(),
                    "{key} out_of_scope 必须写原因"
                );
            }
            "uncovered" => panic!("{key} 仍未被覆盖且未登记为 out_of_scope"),
            other => panic!("{key} 状态非法: {other}"),
        }
    }
    let summary = &report["summary"]["by_status"];
    assert_eq!(summary["covered"], serde_json::json!(covered));
    assert_eq!(summary["partial"], serde_json::json!(partial));
    assert_eq!(summary["out_of_scope"], serde_json::json!(out_of_scope));
    assert_eq!(covered + partial + out_of_scope, classes.len());
}
