use jianying_cli::domain::{
    DraftProject, EditOperation, FrameRate, Material, MaterialId, Segment, SegmentId, TimeRange,
    Timeline, Track, TrackKind,
};

#[test]
fn typed_segments_reject_incompatible_track_types() {
    let text = Segment::text(
        SegmentId::new("subtitle-1").unwrap(),
        TimeRange::new(0, 1_000_000).unwrap(),
        "你好，剪映".to_owned(),
    )
    .unwrap();
    let result = Track::new("video-main", TrackKind::Video, vec![text]);
    assert!(result.is_err());
}

#[test]
fn project_requires_referenced_materials() {
    let missing = MaterialId::new("missing-video").unwrap();
    let video = Segment::video(
        SegmentId::new("video-1").unwrap(),
        TimeRange::new(0, 1_000_000).unwrap(),
        missing,
        TimeRange::new(0, 1_000_000).unwrap(),
    )
    .unwrap();
    let track = Track::new("video-main", TrackKind::Video, vec![video]).unwrap();
    let timeline = Timeline::new(vec![track]).unwrap();
    let result = DraftProject::new(
        "missing-material",
        1920,
        1080,
        FrameRate::new(30, 1).unwrap(),
        timeline,
        vec![],
    );
    assert!(result.is_err());
}

#[test]
fn lossless_domain_round_trip_preserves_segment_variant() {
    let material_id = MaterialId::new("video-a").unwrap();
    let material = Material::video(material_id.clone(), "media/a.mp4".into());
    let video = Segment::video(
        SegmentId::new("video-1").unwrap(),
        TimeRange::new(0, 1_000_000).unwrap(),
        material_id,
        TimeRange::new(0, 1_000_000).unwrap(),
    )
    .unwrap();
    let track = Track::new("video-main", TrackKind::Video, vec![video]).unwrap();
    let timeline = Timeline::new(vec![track]).unwrap();
    let project = DraftProject::new(
        "round-trip",
        1920,
        1080,
        FrameRate::new(30, 1).unwrap(),
        timeline,
        vec![material],
    )
    .unwrap();

    let encoded = serde_json::to_string(&project).unwrap();
    let decoded: DraftProject = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, project);
    assert!(matches!(
        decoded.timeline().tracks()[0].segments()[0],
        Segment::Video { .. }
    ));
}

#[test]
fn frame_quantization_is_explicit_and_bounded() {
    let fps = FrameRate::new(30, 1).unwrap();
    let source = TimeRange::new(10_000, 50_000).unwrap();
    let report = fps.quantize_containing(source, 30_000).unwrap();
    assert_eq!(report.quantized().start_us(), 0);
    assert_eq!(report.quantized().end_us(), 66_666);
    assert!(report.start_delta_us() <= 0);
    assert!(report.end_delta_us() >= 0);

    assert!(fps.quantize_containing(source, 1_000).is_err());
}

#[test]
fn edit_operations_have_a_stable_tagged_round_trip() {
    let operation = EditOperation::MoveSegment {
        segment_id: SegmentId::new("video-1").unwrap(),
        target: TimeRange::new(2_000_000, 500_000).unwrap(),
    };
    let encoded = serde_json::to_value(&operation).unwrap();
    assert_eq!(encoded["operation"], "move_segment");
    assert_eq!(
        serde_json::from_value::<EditOperation>(encoded).unwrap(),
        operation
    );
}

#[test]
fn v1_plan_converts_into_the_unified_domain_model() {
    let plan: jianying_cli::plan::Plan = serde_json::from_value(serde_json::json!({
        "schema": "jianying-cli-plan/v1",
        "name": "compat",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [
            {"type": "video", "segments": [{
                "start_us": 0,
                "duration_us": 1_000_000,
                "source": "a.mp4"
            }]},
            {"type": "text", "segments": [{
                "start_us": 0,
                "duration_us": 1_000_000,
                "text": "标题"
            }]}
        ]
    }))
    .unwrap();
    plan.validate().unwrap();

    let project = jianying_cli::domain_compat::from_v1_plan(&plan).unwrap();
    assert_eq!(project.timeline().tracks().len(), 2);
    assert_eq!(project.materials().len(), 1);
}

#[test]
fn v1_conversion_keeps_duplicate_display_names_without_id_collisions() {
    let plan: jianying_cli::plan::Plan = serde_json::from_value(serde_json::json!({
        "schema": "jianying-cli-plan/v1",
        "name": "duplicate-track-names",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [
            {"type": "video", "name": "画面", "segments": [{
                "start_us": 0, "duration_us": 1_000_000, "source": "a.mp4"
            }]},
            {"type": "video", "name": "画面", "segments": [{
                "start_us": 0, "duration_us": 1_000_000, "source": "b.mp4"
            }]}
        ]
    }))
    .unwrap();
    plan.validate().unwrap();

    let project = jianying_cli::domain_compat::from_v1_plan(&plan).unwrap();
    assert_eq!(project.timeline().tracks()[0].id(), "画面-0");
    assert_eq!(project.timeline().tracks()[1].id(), "画面-1");
}
