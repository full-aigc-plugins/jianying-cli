use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "jianying-home-folder-cli-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    write_json(
        &root.join("folder_meta_info.json"),
        &json!({
            "version": "1.0",
            "timestamp": "2026-09-22T20:00:00",
            "future_root": {"keep": true},
            "folders": [{
                "id": "folder-one",
                "name": "新建文件夹",
                "parentId": "",
                "createdTime": "2026-09-22T19:00:00",
                "modifiedTime": "2026-09-22T19:00:00",
                "future_folder": [1, 2, 3]
            }]
        }),
    );
    write_json(
        &root.join("draft_folder_mappings.json"),
        &json!({
            "version": "1.0",
            "timestamp": "2026-09-22T20:00:00",
            "future_mappings_root": "preserve",
            "mappings": [{
                "draftId": "unrelated-draft",
                "folderId": "",
                "mappedTime": "2026-09-22T18:00:00",
                "future_mapping": 7
            }]
        }),
    );
    write_json(
        &root.join("recycle_bin.json"),
        &json!({
            "version": "1.0",
            "timestamp": "2026-09-22T20:00:00",
            "future_recycle_root": {"keep": "yes"},
            "recycled_folders": []
        }),
    );
    write_json(
        &root.join("draft_mapping_recycle_bin.json"),
        &json!({
            "version": "1.0",
            "timestamp": "2026-09-22T20:00:00",
            "future_mapping_recycle_root": true,
            "recycled_draft_mappings": []
        }),
    );
    (root.clone(), root)
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
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

#[test]
fn empty_folder_recycle_and_restore_are_lossless_and_recoverable() {
    let (sandbox, config_root) = fixture();
    let config = config_root.to_string_lossy();

    let listed = run_ok(&["home", "folder", "list", "--config-root", &config, "--json"]);
    assert_eq!(listed["folders"][0]["id"], "folder-one");
    assert_eq!(listed["folders"][0]["future_folder"], json!([1, 2, 3]));

    let recycled = run_ok(&[
        "home",
        "folder",
        "recycle",
        "folder-one",
        "--config-root",
        &config,
        "--at",
        "2026-09-22T21:00:00",
        "--yes",
        "--json",
    ]);
    assert_eq!(recycled["status"], "recycled");
    assert_eq!(recycled["folder_id"], "folder-one");
    assert_eq!(recycled["before"]["active_folders"], 1);
    assert_eq!(recycled["after"]["active_folders"], 0);
    assert_eq!(recycled["after"]["recycled_folders"], 1);
    assert!(Path::new(recycled["snapshot_path"].as_str().unwrap()).is_dir());
    let recycle_id = recycled["recycle_id"].as_str().unwrap().to_owned();

    let active = read_json(&config_root.join("folder_meta_info.json"));
    let recycle = read_json(&config_root.join("recycle_bin.json"));
    let mappings = read_json(&config_root.join("draft_folder_mappings.json"));
    let recycled_mappings = read_json(&config_root.join("draft_mapping_recycle_bin.json"));
    assert_eq!(active["future_root"], json!({"keep": true}));
    assert_eq!(active["folders"], json!([]));
    assert_eq!(recycle["future_recycle_root"], json!({"keep": "yes"}));
    assert_eq!(
        recycle["recycled_folders"][0]["folder_info"]["future_folder"],
        json!([1, 2, 3])
    );
    assert_eq!(mappings["future_mappings_root"], "preserve");
    assert_eq!(mappings["mappings"][0]["future_mapping"], 7);
    assert_eq!(recycled_mappings["future_mapping_recycle_root"], true);

    let recycled_list = run_ok(&[
        "home",
        "folder",
        "list-recycled",
        "--config-root",
        &config,
        "--json",
    ]);
    assert_eq!(
        recycled_list["recycled_folders"][0]["recycle_id"],
        recycle_id
    );

    let restored = run_ok(&[
        "home",
        "folder",
        "restore",
        &recycle_id,
        "--config-root",
        &config,
        "--at",
        "2026-09-22T21:01:00",
        "--json",
    ]);
    assert_eq!(restored["status"], "restored");
    assert_eq!(restored["folder_id"], "folder-one");
    assert_eq!(restored["before"]["recycled_folders"], 1);
    assert_eq!(restored["after"]["active_folders"], 1);
    assert_eq!(restored["after"]["recycled_folders"], 0);

    let active = read_json(&config_root.join("folder_meta_info.json"));
    let recycle = read_json(&config_root.join("recycle_bin.json"));
    assert_eq!(active["folders"][0]["id"], "folder-one");
    assert_eq!(active["folders"][0]["future_folder"], json!([1, 2, 3]));
    assert_eq!(active["future_root"], json!({"keep": true}));
    assert_eq!(recycle["recycled_folders"], json!([]));
    assert_eq!(recycle["future_recycle_root"], json!({"keep": "yes"}));
    let _ = std::fs::remove_dir_all(sandbox);
}

#[test]
fn create_preserves_unknown_fields_and_rejects_duplicate_names() {
    let (sandbox, config_root) = fixture();
    let config = config_root.to_string_lossy();
    let created = run_ok(&[
        "home",
        "folder",
        "create",
        "交付文件夹",
        "--config-root",
        &config,
        "--at",
        "2026-09-22T21:02:00",
        "--json",
    ]);
    assert_eq!(created["status"], "created");
    assert_eq!(created["folder"]["name"], "交付文件夹");
    assert_eq!(
        read_json(&config_root.join("folder_meta_info.json"))["future_root"],
        json!({"keep": true})
    );

    let before = std::fs::read(config_root.join("folder_meta_info.json")).unwrap();
    let duplicate = run(&[
        "home",
        "folder",
        "create",
        "交付文件夹",
        "--config-root",
        &config,
        "--json",
    ]);
    assert_eq!(duplicate.status.code(), Some(1));
    assert_eq!(
        std::fs::read(config_root.join("folder_meta_info.json")).unwrap(),
        before
    );
    let _ = std::fs::remove_dir_all(sandbox);
}

#[test]
fn rename_preserves_folder_relationships_and_rejects_ambiguous_targets() {
    let (sandbox, config_root) = fixture();
    let config = config_root.to_string_lossy();
    let mut folders = read_json(&config_root.join("folder_meta_info.json"));
    folders["folders"].as_array_mut().unwrap().push(json!({
        "id": "folder-two", "name": "已有名称", "parentId": "",
        "createdTime": "2026-09-22T19:10:00", "modifiedTime": "2026-09-22T19:10:00",
        "future_folder": {"retain": true}
    }));
    write_json(&config_root.join("folder_meta_info.json"), &folders);
    let mut mappings = read_json(&config_root.join("draft_folder_mappings.json"));
    mappings["mappings"].as_array_mut().unwrap().push(json!({
        "draftId": "mapped-draft", "folderId": "folder-one",
        "mappedTime": "2026-09-22T19:20:00", "future_mapping": 9
    }));
    write_json(&config_root.join("draft_folder_mappings.json"), &mappings);

    let result = run_ok(&[
        "home",
        "folder",
        "rename",
        "folder-one",
        "交付素材",
        "--config-root",
        &config,
        "--at",
        "2026-09-23T09:00:00",
        "--json",
    ]);
    assert_eq!(result["status"], "renamed");
    assert_eq!(result["folder_id"], "folder-one");
    assert_eq!(result["old_name"], "新建文件夹");
    assert_eq!(result["new_name"], "交付素材");
    assert!(Path::new(result["snapshot_path"].as_str().unwrap()).is_dir());
    let updated = read_json(&config_root.join("folder_meta_info.json"));
    assert_eq!(updated["folders"][0]["id"], "folder-one");
    assert_eq!(updated["folders"][0]["parentId"], "");
    assert_eq!(updated["folders"][0]["future_folder"], json!([1, 2, 3]));
    assert_eq!(updated["folders"][0]["modifiedTime"], "2026-09-23T09:00:00");
    assert_eq!(updated["folders"][1]["name"], "已有名称");
    assert_eq!(updated["timestamp"], "2026-09-23T09:00:00");
    assert_eq!(
        read_json(&config_root.join("draft_folder_mappings.json")),
        mappings
    );

    let names = [
        "folder_meta_info.json",
        "draft_folder_mappings.json",
        "recycle_bin.json",
        "draft_mapping_recycle_bin.json",
    ];
    let before: Vec<Vec<u8>> = names
        .iter()
        .map(|name| std::fs::read(config_root.join(name)).unwrap())
        .collect();
    for (id, name) in [
        ("missing", "允许的名称"),
        ("folder-one", "已有名称"),
        ("folder-one", "  "),
        ("folder-one", "非法\n名称"),
    ] {
        let failed = run(&[
            "home",
            "folder",
            "rename",
            id,
            name,
            "--config-root",
            &config,
            "--json",
        ]);
        assert_eq!(failed.status.code(), Some(1));
        for (index, file) in names.iter().enumerate() {
            assert_eq!(
                std::fs::read(config_root.join(file)).unwrap(),
                before[index]
            );
        }
    }
    let _ = std::fs::remove_dir_all(sandbox);
}

#[test]
fn nonempty_or_conflicting_folder_operations_fail_without_writes() {
    let (sandbox, config_root) = fixture();
    let config = config_root.to_string_lossy();
    let mut mappings = read_json(&config_root.join("draft_folder_mappings.json"));
    mappings["mappings"].as_array_mut().unwrap().push(json!({
        "draftId": "draft-in-folder",
        "folderId": "folder-one",
        "mappedTime": "2026-09-22T20:10:00"
    }));
    write_json(&config_root.join("draft_folder_mappings.json"), &mappings);
    let names = [
        "folder_meta_info.json",
        "draft_folder_mappings.json",
        "recycle_bin.json",
        "draft_mapping_recycle_bin.json",
    ];
    let before: Vec<Vec<u8>> = names
        .iter()
        .map(|name| std::fs::read(config_root.join(name)).unwrap())
        .collect();

    let output = run(&[
        "home",
        "folder",
        "recycle",
        "folder-one",
        "--config-root",
        &config,
        "--yes",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("nonempty_folder_schema_unverified"));
    for (index, name) in names.iter().enumerate() {
        assert_eq!(
            std::fs::read(config_root.join(name)).unwrap(),
            before[index]
        );
    }
    let _ = std::fs::remove_dir_all(sandbox);
}

#[test]
fn confirmation_and_host_read_only_guards_leave_config_byte_identical() {
    let (sandbox, config_root) = fixture();
    let config = config_root.to_string_lossy();
    let names = [
        "folder_meta_info.json",
        "draft_folder_mappings.json",
        "recycle_bin.json",
        "draft_mapping_recycle_bin.json",
    ];
    let before: Vec<Vec<u8>> = names
        .iter()
        .map(|name| std::fs::read(config_root.join(name)).unwrap())
        .collect();

    let unconfirmed = run(&[
        "home",
        "folder",
        "recycle",
        "folder-one",
        "--config-root",
        &config,
        "--json",
    ]);
    assert_eq!(unconfirmed.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&unconfirmed.stdout).contains("home_folder_confirmation_required")
    );

    let read_only = run(&[
        "--host-read-only",
        "home",
        "folder",
        "create",
        "blocked",
        "--config-root",
        &config,
        "--json",
    ]);
    assert_eq!(read_only.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&read_only.stdout).contains("host_read_only"));
    let rename_read_only = run(&[
        "--host-read-only",
        "home",
        "folder",
        "rename",
        "folder-one",
        "不可写",
        "--config-root",
        &config,
        "--json",
    ]);
    assert_eq!(rename_read_only.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&rename_read_only.stdout).contains("host_read_only"));
    for (index, name) in names.iter().enumerate() {
        assert_eq!(
            std::fs::read(config_root.join(name)).unwrap(),
            before[index]
        );
    }
    let _ = std::fs::remove_dir_all(sandbox);
}

#[test]
fn machine_catalog_exposes_supported_home_folder_commands_with_exact_access() {
    let catalog = run_ok(&["commands", "--json"]);
    let commands = catalog["commands"].as_array().unwrap();
    let expected = [
        ("home folder list", "read"),
        ("home folder create", "write"),
        ("home folder recycle", "write"),
        ("home folder list-recycled", "read"),
        ("home folder restore", "write"),
    ];
    for (path, access) in expected {
        let command = commands
            .iter()
            .find(|command| command["path"] == path)
            .unwrap_or_else(|| panic!("missing command {path}"));
        assert_eq!(command["group"], "home");
        assert_eq!(command["access"], access);
        assert_eq!(command["status"], "supported");
        assert_eq!(command["platforms"], json!(["macos", "windows"]));
    }
    let rename = commands
        .iter()
        .find(|command| command["path"] == "home folder rename")
        .expect("missing home folder rename");
    assert_eq!(rename["group"], "home");
    assert_eq!(rename["access"], "write");
    assert_eq!(rename["status"], "partial");
    assert_eq!(rename["platforms"], json!(["macos", "windows"]));
}
