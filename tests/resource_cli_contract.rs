//! 用户资源库命令合同：从草稿反演、按来源幂等写入、跨来源合并与检索。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
#[cfg(any(unix, windows))]
use std::sync::OnceLock;

fn run(args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jianying"));
    command.args(args);
    #[cfg(unix)]
    command.env("PATH", isolated_process_path());
    #[cfg(windows)]
    command.env("JIANYING_CLI_PROCESS_LIST", empty_process_listing());
    command.output().unwrap()
}

#[cfg(unix)]
fn isolated_process_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("jianying-resource-bin-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let ps = root.join("ps");
        fs::write(&ps, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&ps, fs::Permissions::from_mode(0o700)).unwrap();
        root.to_string_lossy().into_owned()
    })
}

#[cfg(windows)]
fn empty_process_listing() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = std::env::temp_dir().join(format!(
            "jianying-resource-processes-{}.txt",
            std::process::id()
        ));
        fs::write(&path, "Image Name,PID\r\n").unwrap();
        path.to_string_lossy().into_owned()
    })
}

fn run_ok(args: &[&str]) -> Value {
    let output = run(args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

fn temp_root(label: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("jianying-resource-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_draft(root: &Path, name: &str, id: &str, effects: Value) {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    let content = json!({
        "id": id,
        "update_time": 1_700_000_000 + effects.as_array().map(|a| a.len()).unwrap_or(0) as i64,
        "materials": { "effects": effects },
    });
    fs::write(
        dir.join("draft_content.json"),
        serde_json::to_string_pretty(&content).unwrap(),
    )
    .unwrap();
}

#[test]
fn mine_aggregates_plain_drafts_and_ignores_mirror_copies() {
    let root = temp_root("mine");
    write_draft(
        &root,
        "a",
        "draft-a",
        json!([
            {"type":"text_shape","name":"花字-荧光","effect_id":"111","resource_id":"222"},
            {"type":"filter","name":"清晰","effect_id":"333","resource_id":"444"}
        ]),
    );
    write_draft(
        &root,
        "b",
        "draft-b",
        json!([{"type":"text_shape","name":"花字-荧光","effect_id":"111","resource_id":"222"}]),
    );
    // 同一草稿的镜像副本必须被跳过，否则 uses 会翻倍。
    let mirror = root.join("b").join("Timelines").join("uuid");
    fs::create_dir_all(&mirror).unwrap();
    fs::copy(
        root.join("b").join("draft_content.json"),
        mirror.join("draft_content.json"),
    )
    .unwrap();

    let mined = run_ok(&["resource", "mine", &root.to_string_lossy(), "--json"]);
    assert_eq!(mined["drafts_scanned"], json!(2));
    assert_eq!(mined["drafts_with_resources"], json!(2));
    assert_eq!(mined["drafts_encrypted"], json!(0));
    let entries = mined["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    let text_shape = entries
        .iter()
        .find(|entry| entry["kind"] == "text_shape")
        .unwrap();
    assert_eq!(text_shape["uses"], json!(2), "两个草稿各用一次");
    assert_eq!(text_shape["drafts"], json!(2));
    assert_eq!(text_shape["name"], json!("花字-荧光"));
    assert_eq!(text_shape["effect_id"], json!("111"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mine_tolerates_bom_and_reports_encrypted_drafts_honestly() {
    let root = temp_root("mine-mixed");
    write_draft(
        &root,
        "plain",
        "draft-plain",
        json!([{"type":"filter","name":"暖色","effect_id":"999"}]),
    );
    // 带 BOM 的合法 JSON
    let bom = root.join("bom");
    fs::create_dir_all(&bom).unwrap();
    let body = serde_json::to_string(&json!({
        "id": "draft-bom",
        "materials": {"effects": [{"type":"filter","name":"冷色","effect_id":"888"}]}
    }))
    .unwrap();
    fs::write(bom.join("draft_content.json"), format!("\u{feff}{body}")).unwrap();
    // 剪映 11.6 的加密草稿：不是 JSON，也不应让命令失败
    let enc = root.join("encrypted");
    fs::create_dir_all(&enc).unwrap();
    fs::write(
        enc.join("draft_content.json"),
        "79aEUSlIq3v0Zr8kQm2Xc1TbYpNwHd5VfGjKoL6sA9uE=",
    )
    .unwrap();

    let mined = run_ok(&["resource", "mine", &root.to_string_lossy(), "--json"]);
    assert_eq!(mined["drafts_scanned"], json!(3));
    assert_eq!(mined["drafts_with_resources"], json!(2));
    assert_eq!(mined["drafts_encrypted"], json!(1));
    assert_eq!(mined["drafts_unreadable"], json!(0));
    assert_eq!(mined["entries"].as_array().unwrap().len(), 2);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn catalog_write_is_idempotent_per_source_and_survives_list_and_resolve() {
    let root = temp_root("catalog");
    let drafts = root.join("drafts");
    fs::create_dir_all(&drafts).unwrap();
    write_draft(
        &drafts,
        "a",
        "draft-a",
        json!([{"type":"text_shape","name":"花字-荧光","effect_id":"111","resource_id":"222"}]),
    );
    let catalog = root.join("resources.json");
    let catalog_arg = catalog.to_string_lossy().into_owned();

    run_ok(&[
        "resource",
        "mine",
        &drafts.to_string_lossy(),
        "--catalog",
        &catalog_arg,
        "--write",
        "--json",
    ]);
    // 重复扫描同一来源不能把 uses 越滚越大
    run_ok(&[
        "resource",
        "mine",
        &drafts.to_string_lossy(),
        "--catalog",
        &catalog_arg,
        "--write",
        "--json",
    ]);
    let listed = run_ok(&["resource", "list", "--catalog", &catalog_arg, "--json"]);
    assert_eq!(listed["total"], json!(1));
    assert_eq!(listed["entries"][0]["uses"], json!(1));
    assert_eq!(listed["summary"]["sources"], json!(1));

    let resolved = run_ok(&[
        "resource",
        "resolve",
        "荧光",
        "--catalog",
        &catalog_arg,
        "--json",
    ]);
    assert_eq!(resolved["matches"][0]["effect_id"], json!("111"));

    let filtered = run_ok(&[
        "resource",
        "list",
        "--catalog",
        &catalog_arg,
        "--kind",
        "filter",
        "--json",
    ]);
    assert_eq!(filtered["total"], json!(0));

    let out = root.join("export.json");
    run_ok(&[
        "resource",
        "export",
        "--catalog",
        &catalog_arg,
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let exported: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(exported["schema"], json!("jianying-resource-library/v1"));
    assert_eq!(exported["entries"].as_array().unwrap().len(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn import_merges_an_external_bundle_as_its_own_source() {
    let root = temp_root("import");
    let catalog = root.join("resources.json");
    let catalog_arg = catalog.to_string_lossy().into_owned();
    let bundle = root.join("bundle.json");
    fs::write(
        &bundle,
        serde_json::to_string(&json!({"entries": [
            {"type":"video_effect","name":"复古胶片","effect_id":"777","resource_id":"666","uses":4},
            {"kind":"transition","name":"推移","effect_id":"555"}
        ]}))
        .unwrap(),
    )
    .unwrap();

    let imported = run_ok(&[
        "resource",
        "import",
        &bundle.to_string_lossy(),
        "--catalog",
        &catalog_arg,
        "--json",
    ]);
    assert_eq!(imported["entries"], json!(2));
    assert_eq!(imported["summary"]["sources"], json!(1));

    let listed = run_ok(&["resource", "list", "--catalog", &catalog_arg, "--json"]);
    assert_eq!(listed["total"], json!(2));
    let kinds: Vec<&str> = listed["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"video_effect") && kinds.contains(&"transition"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_attaches_a_library_resource_once_and_keeps_the_draft_valid() {
    let root = temp_root("apply");
    let catalog = root.join("resources.json");
    let catalog_arg = catalog.to_string_lossy().into_owned();
    let bundle = root.join("bundle.json");
    fs::write(
        &bundle,
        serde_json::to_string(&json!({"entries": [
            {"type":"text_shape","name":"花字-荧光","effect_id":"111","resource_id":"222","md5":"abc"}
        ]}))
        .unwrap(),
    )
    .unwrap();
    run_ok(&[
        "resource",
        "import",
        &bundle.to_string_lossy(),
        "--catalog",
        &catalog_arg,
        "--json",
    ]);

    let plan = root.join("plan.json");
    fs::write(
        &plan,
        serde_json::to_string(&json!({
            "schema": "jianying-cli-plan/v1",
            "name": "apply-target",
            "canvas": {"width": 1920, "height": 1080, "fps": 30},
            "tracks": [{"type": "text", "segments": [{
                "start_us": 0, "duration_us": 1_000_000, "text": "目标"
            }]}]
        }))
        .unwrap(),
    )
    .unwrap();
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    let segment = run_ok(&[
        "timeline",
        "segments",
        &draft.to_string_lossy(),
        "--track",
        "text",
        "--json",
    ])[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let applied = run_ok(&[
        "resource",
        "apply",
        &draft.to_string_lossy(),
        &segment,
        "荧光",
        "--catalog",
        &catalog_arg,
        "--json",
    ]);
    assert_eq!(applied["applied"], json!(true));
    assert_eq!(applied["kind"], json!("text_shape"));
    let material_id = applied["material_id"].as_str().unwrap().to_owned();
    assert_eq!(applied["resource"]["name"], json!("花字-荧光"));

    // 再挂一次必须幂等：不新增素材，返回 already-applied
    let again = run_ok(&[
        "resource",
        "apply",
        &draft.to_string_lossy(),
        &segment,
        "荧光",
        "--catalog",
        &catalog_arg,
        "--json",
    ]);
    assert_eq!(again["applied"], json!(false));
    assert_eq!(again["reason"], json!("already-applied"));
    assert_eq!(again["material_id"], json!(material_id));

    let content: Value =
        serde_json::from_str(&fs::read_to_string(draft.join("draft_content.json")).unwrap())
            .unwrap();
    let shapes: Vec<&Value> = content["materials"]["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["type"] == "text_shape")
        .collect();
    assert_eq!(shapes.len(), 1, "重复 apply 不能产生重复素材");
    assert_eq!(shapes[0]["effect_id"], json!("111"));
    assert_eq!(shapes[0]["resource_id"], json!("222"));
    assert_eq!(
        run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_rejects_resources_it_cannot_attach_and_ambiguous_names() {
    let root = temp_root("apply-negative");
    let catalog = root.join("resources.json");
    let catalog_arg = catalog.to_string_lossy().into_owned();
    let bundle = root.join("bundle.json");
    fs::write(
        &bundle,
        serde_json::to_string(&json!({"entries": [
            {"type":"filter","name":"清晰","effect_id":"333","resource_id":"444"},
            {"type":"text_shape","name":"花字-荧光","effect_id":"111","resource_id":"222"},
            {"type":"video_effect","name":"花字-荧光","effect_id":"555","resource_id":"666"}
        ]}))
        .unwrap(),
    )
    .unwrap();
    run_ok(&[
        "resource",
        "import",
        &bundle.to_string_lossy(),
        "--catalog",
        &catalog_arg,
        "--json",
    ]);

    let plan = root.join("plan.json");
    fs::write(
        &plan,
        serde_json::to_string(&json!({
            "schema": "jianying-cli-plan/v1",
            "name": "apply-negative",
            "canvas": {"width": 1920, "height": 1080, "fps": 30},
            "tracks": [{"type": "text", "segments": [{
                "start_us": 0, "duration_us": 1_000_000, "text": "目标"
            }]}]
        }))
        .unwrap(),
    )
    .unwrap();
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    let segment = run_ok(&[
        "timeline",
        "segments",
        &draft.to_string_lossy(),
        "--track",
        "text",
        "--json",
    ])[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // filter 不能用 apply 挂载，必须引导到 timeline filter
    let filter = run(&[
        "resource",
        "apply",
        &draft.to_string_lossy(),
        &segment,
        "清晰",
        "--catalog",
        &catalog_arg,
        "--json",
    ]);
    assert_eq!(filter.status.code(), Some(1));
    let filter_err = String::from_utf8_lossy(&filter.stdout);
    assert!(
        filter_err.contains("timeline filter"),
        "错误信息必须指向替代命令: {filter_err}"
    );

    // 同名跨种类 → 歧义，必须要求 --kind
    let ambiguous = run(&[
        "resource",
        "apply",
        &draft.to_string_lossy(),
        &segment,
        "花字-荧光",
        "--catalog",
        &catalog_arg,
        "--json",
    ]);
    assert_eq!(ambiguous.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&ambiguous.stdout).contains("ambiguous"));
    let _ = fs::remove_dir_all(root);
}
