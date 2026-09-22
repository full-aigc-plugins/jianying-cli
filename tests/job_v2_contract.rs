use jianying_cli::domain::{
    DraftProject, EditOperation, FrameRate, Material, MaterialId, Segment, SegmentId, TimeRange,
    Timeline, Track, TrackKind,
};
use jianying_cli::schema::{
    ExportKind, ExportRequest, JobOperation, JobV2, ProjectTarget, JOB_V2_SCHEMA,
};
use std::path::PathBuf;

fn project() -> DraftProject {
    let material_id = MaterialId::new("video-a").unwrap();
    let segment = Segment::video(
        SegmentId::new("video-1").unwrap(),
        TimeRange::new(0, 1_000_000).unwrap(),
        material_id.clone(),
        TimeRange::new(0, 1_000_000).unwrap(),
    )
    .unwrap();
    let timeline = Timeline::new(vec![Track::new(
        "video-main",
        TrackKind::Video,
        vec![segment],
    )
    .unwrap()])
    .unwrap();
    DraftProject::new(
        "job-v2",
        1920,
        1080,
        FrameRate::new(30, 1).unwrap(),
        timeline,
        vec![Material::video(material_id, "a.mp4".into())],
    )
    .unwrap()
}

#[test]
fn create_job_has_a_stable_v2_envelope() {
    let job = JobV2::create(project()).unwrap();
    assert_eq!(job.schema(), JOB_V2_SCHEMA);
    assert_eq!(job.operation(), JobOperation::Create);
    let value = serde_json::to_value(&job).unwrap();
    assert_eq!(value["schema"], "jianying-job/v2");
    assert_eq!(value["operation"], "create");
    assert_eq!(value["project"]["type"], "new");
    JobV2::parse(&serde_json::to_string(&value).unwrap()).unwrap();
}

#[test]
fn all_required_operations_have_valid_constructors() {
    let source = PathBuf::from("drafts/source");
    let copy = PathBuf::from("work/source-copy");
    JobV2::edit_existing(
        source.clone(),
        copy,
        vec![EditOperation::ReplaceText {
            segment_id: SegmentId::new("caption-1").unwrap(),
            text: "新字幕".to_owned(),
        }],
    )
    .unwrap();
    JobV2::existing(JobOperation::Inspect, source.clone()).unwrap();
    JobV2::existing(JobOperation::Verify, source.clone()).unwrap();
    JobV2::existing(JobOperation::Publish, source.clone()).unwrap();
    JobV2::export_existing(
        source,
        ExportRequest::new(ExportKind::Proxy, "preview.mp4".into(), false).unwrap(),
    )
    .unwrap();
    JobV2::batch(vec![JobV2::create(project()).unwrap()]).unwrap();
}

#[test]
fn invalid_operation_shapes_and_unknown_schema_are_rejected() {
    assert!(JobV2::existing(JobOperation::Create, "drafts/a".into()).is_err());
    assert!(JobV2::edit_existing(
        "same".into(),
        "same".into(),
        vec![EditOperation::RemoveSegment {
            segment_id: SegmentId::new("segment-1").unwrap(),
        }]
    )
    .is_err());
    assert!(JobV2::edit_existing("source".into(), "copy".into(), vec![]).is_err());
    assert!(JobV2::batch(vec![]).is_err());

    let mut value = serde_json::to_value(JobV2::create(project()).unwrap()).unwrap();
    value["schema"] = serde_json::json!("jianying-job/v99");
    assert!(JobV2::parse(&value.to_string()).is_err());

    let mut invalid_domain = serde_json::to_value(JobV2::create(project()).unwrap()).unwrap();
    invalid_domain["project"]["project"]["width"] = serde_json::json!(0);
    assert!(JobV2::parse(&invalid_domain.to_string()).is_err());

    let empty_project = serde_json::json!({
        "schema": "jianying-job/v2",
        "operation": "create",
        "project": {"type": "new", "project": {}}
    });
    assert!(JobV2::parse(&empty_project.to_string()).is_err());

    let invalid_edit_operation = serde_json::json!({
        "schema":"jianying-job/v2","operation":"edit",
        "project":{"type":"existing","source":"source","output":"copy"},
        "operations":[{"operation":"move_segment","segment_id":"",
            "target":{"start_us":-1,"duration_us":0}}]
    });
    assert!(JobV2::parse(&invalid_edit_operation.to_string()).is_err());
}

#[test]
fn track_edit_operations_have_closed_round_trip_contracts() {
    let operations = [
        serde_json::json!({
            "operation":"add_track",
            "track_id":"video-overlay",
            "name":"叠加画面",
            "kind":"video",
            "index":1
        }),
        serde_json::json!({
            "operation":"reorder_track",
            "track_id":"video-overlay",
            "index":0
        }),
        serde_json::json!({
            "operation":"remove_track",
            "track_id":"video-overlay"
        }),
    ];
    for value in operations {
        let operation: EditOperation = serde_json::from_value(value.clone()).unwrap();
        operation.validate().unwrap();
        assert_eq!(serde_json::to_value(operation).unwrap(), value);
    }

    for invalid in [
        serde_json::json!({"operation":"add_track","track_id":"","kind":"video"}),
        serde_json::json!({"operation":"add_track","track_id":"v","name":" ","kind":"video"}),
        serde_json::json!({"operation":"remove_track","track_id":""}),
        serde_json::json!({"operation":"reorder_track","track_id":"","index":0}),
    ] {
        let operation = serde_json::from_value::<EditOperation>(invalid).unwrap();
        assert!(operation.validate().is_err());
    }
}

#[test]
fn checked_in_json_schema_covers_the_seven_operations() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/jianying-job-v2.schema.json")).unwrap();
    let values = schema["properties"]["operation"]["enum"]
        .as_array()
        .unwrap();
    assert_eq!(values.len(), 7);
    for operation in [
        "create", "edit", "inspect", "verify", "publish", "export", "batch",
    ] {
        assert!(values.iter().any(|value| value == operation));
    }
    assert_eq!(
        schema["properties"]["project"]["oneOf"][0]["properties"]["project"]["$ref"],
        "#/$defs/draftProject"
    );
    let required = schema["$defs"]["draftProject"]["required"]
        .as_array()
        .unwrap();
    for field in [
        "name",
        "width",
        "height",
        "frame_rate",
        "timeline",
        "materials",
    ] {
        assert!(required.iter().any(|value| value == field));
    }
    assert_eq!(
        schema["properties"]["operations"]["items"]["$ref"],
        "#/$defs/editOperation"
    );
    let edit_operations = schema["$defs"]["editOperation"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|operation| operation["properties"]["operation"]["const"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for operation in [
        "add_material",
        "add_segment",
        "add_track",
        "remove_track",
        "reorder_track",
        "remove_segment",
        "move_segment",
        "replace_text",
    ] {
        assert!(
            edit_operations.contains(operation),
            "Job v2 schema is missing {operation}"
        );
    }
    assert_eq!(
        schema["properties"]["compatibility"]["properties"]["schema"]["enum"],
        serde_json::json!(["jianying-cli-plan/v1", "capcut-cli-compile/v1"])
    );
    assert_eq!(
        schema["allOf"][0]["then"]["anyOf"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for field in [
        "keyframes",
        "mask",
        "chroma",
        "background_filling",
        "mix_mode",
        "animation_in",
        "animation_out",
        "animation_group",
        "transition_out",
        "fade",
    ] {
        assert!(
            schema["$defs"]["videoSegment"]["properties"]
                .get(field)
                .is_some(),
            "video domain schema is missing {field}"
        );
    }
    for field in ["keyframes", "fade", "audio_effects"] {
        assert!(
            schema["$defs"]["audioSegment"]["properties"]
                .get(field)
                .is_some(),
            "audio domain schema is missing {field}"
        );
    }
    let text_properties = &schema["$defs"]["segment"]["oneOf"][2]["properties"];
    for field in [
        "keyframes",
        "animation_in",
        "animation_out",
        "animation_group",
        "size",
        "background",
        "shadow",
        "styles",
        "text_effect",
        "bubble",
    ] {
        assert!(
            text_properties.get(field).is_some(),
            "text domain schema is missing {field}"
        );
    }
}

#[test]
fn compile_compatibility_can_create_without_duplicate_project() {
    let payload = serde_json::json!({
        "name":"compiled",
        "tracks":[{"type":"text","items":[{"text":"hello","start":0,"duration":1}]}]
    });
    let job = JobV2::create_from_compatibility(
        jianying_schema::CompatibilityInput::compile_v1(payload.clone()).unwrap(),
    )
    .unwrap();
    let value = serde_json::to_value(&job).unwrap();
    assert!(value.get("project").is_none());
    assert_eq!(value["compatibility"]["schema"], "capcut-cli-compile/v1");
    assert_eq!(value["compatibility"]["payload"], payload);
    JobV2::parse(&value.to_string()).unwrap();
}

#[test]
fn checked_in_create_example_parses_with_the_runtime_contract() {
    JobV2::parse(include_str!("../examples/job-v2-create.json")).unwrap();
}

#[test]
fn v1_plan_converts_to_a_valid_v2_create_job() {
    let plan: jianying_cli::plan::Plan = serde_json::from_value(serde_json::json!({
        "schema": "jianying-cli-plan/v1",
        "name": "v1-to-v2",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "video", "segments": [{
            "start_us": 0, "duration_us": 1_000_000, "source": "a.mp4"
        }]}]
    }))
    .unwrap();
    let job = jianying_cli::domain_compat::job_from_v1_plan(&plan).unwrap();
    assert_eq!(job.operation(), JobOperation::Create);
    assert!(matches!(job.project(), Some(ProjectTarget::New { .. })));
}

#[test]
fn all_frozen_v1_fixtures_survive_the_v2_compatibility_envelope() {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/parity/scenarios");
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(&fixture_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    fixtures.sort();
    assert_eq!(fixtures.len(), 55);

    for fixture in fixtures {
        let mut scenario: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
        let mut plan_value = scenario["plan"].take();
        jianying_cli::tim::preprocess(&mut plan_value);
        let plan: jianying_cli::plan::Plan = serde_json::from_value(plan_value).unwrap();
        plan.validate().unwrap();
        let job = jianying_cli::domain_compat::job_from_v1_plan(&plan).unwrap();
        let compatibility = job.compatibility().expect("v1 conversion must preserve v1");
        assert_eq!(compatibility.schema(), "jianying-cli-plan/v1");
        let restored: jianying_cli::plan::Plan =
            serde_json::from_value(compatibility.payload().clone()).unwrap();
        restored.validate().unwrap();
        assert_eq!(
            serde_json::to_value(&plan).unwrap(),
            serde_json::to_value(&restored).unwrap(),
            "{}",
            fixture.display()
        );
    }
}
