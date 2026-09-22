use jianying_cli::domain::{
    Animation, AudioEffect, AudioEffects, BackgroundFilling, BlendMode, ChromaKey, ClipSettings,
    CropSettings, DraftProject, EditOperation, Fade, FrameRate, KeyframePoint, Keyframes, Mask,
    Material, MaterialId, Segment, SegmentId, TextBackground, TextShadow, TextStyle, TimeRange,
    Timeline, Track, TrackKind, Transform, Transition,
};
use std::collections::BTreeMap;

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
    assert_eq!(project.timeline().tracks()[0].name(), Some("画面"));
    assert_eq!(project.timeline().tracks()[1].name(), Some("画面"));

    let projected = jianying_cli::job_runner::plan_from_project(&project).unwrap();
    assert_eq!(projected.tracks[0].name.as_deref(), Some("画面"));
    assert_eq!(projected.tracks[1].name.as_deref(), Some("画面"));
}

#[test]
fn clip_crop_and_transform_are_validated_domain_values() {
    assert!(ClipSettings::new(0.0, 1.0, None).is_err());
    assert!(Transform::new(Some(0.0), None, None, None, None).is_err());
    assert!(CropSettings::new([0.0, 0.0, 1.2, 0.0, 0.0, 1.0, 1.0, 1.0]).is_err());

    let clip = ClipSettings::new(1.25, 0.8, Some(true)).unwrap();
    let transform =
        Transform::new(Some(1.2), Some(0.1), Some(-0.2), Some(15.0), Some(0.7)).unwrap();
    let crop = CropSettings::new([0.1, 0.0, 0.9, 0.0, 0.1, 1.0, 0.9, 1.0]).unwrap();
    assert_eq!(clip.change_pitch(), Some(true));
    assert_eq!(transform.opacity(), Some(0.7));
    assert_eq!(crop.upper_left_x(), 0.1);
}

#[test]
fn v1_conversion_promotes_clip_crop_and_transform_without_changing_job_shape() {
    let plan: jianying_cli::plan::Plan = serde_json::from_value(serde_json::json!({
        "schema": "jianying-cli-plan/v1",
        "name": "typed-visuals",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "video", "segments": [{
            "start_us": 0,
            "duration_us": 1_000_000,
            "source": "a.mp4",
            "speed": 1.25,
            "volume": 0.8,
            "change_pitch": true,
            "scale": 1.2,
            "x": 0.1,
            "y": -0.2,
            "rotation": 15.0,
            "opacity": 0.7,
            "crop": {
                "upper_left_x": 0.1, "upper_left_y": 0.0,
                "upper_right_x": 0.9, "upper_right_y": 0.0,
                "lower_left_x": 0.1, "lower_left_y": 1.0,
                "lower_right_x": 0.9, "lower_right_y": 1.0
            }
        }]}]
    }))
    .unwrap();
    plan.validate().unwrap();

    let project = jianying_cli::domain_compat::from_v1_plan(&plan).unwrap();
    let segment = &project.timeline().tracks()[0].segments()[0];
    let Segment::Video {
        clip,
        transform,
        crop,
        ..
    } = segment
    else {
        panic!("expected video segment");
    };
    assert_eq!(clip.speed(), 1.25);
    assert_eq!(clip.volume(), 0.8);
    assert_eq!(clip.change_pitch(), Some(true));
    assert_eq!(transform.scale(), Some(1.2));
    assert_eq!(crop.as_ref().unwrap().lower_right_x(), 0.9);

    let encoded = serde_json::to_value(segment).unwrap();
    assert_eq!(encoded["speed"], 1.25);
    assert_eq!(encoded["scale"], 1.2);
    assert_eq!(encoded["crop"]["lower_right_x"], 0.9);
    assert!(encoded.get("clip").is_none());
    assert!(encoded.get("transform").is_none());
}

#[test]
fn remaining_advanced_settings_are_independent_validated_values() {
    let keyframes = Keyframes::new(BTreeMap::from([(
        "scale".to_owned(),
        vec![
            KeyframePoint::new(0, 1.0).unwrap(),
            KeyframePoint::new(500_000, 1.2).unwrap(),
        ],
    )]))
    .unwrap();
    assert_eq!(keyframes.channels()["scale"].len(), 2);
    assert!(KeyframePoint::new(-1, 1.0).is_err());

    let mask = Mask::new("线性", 0.0, 0.0, 0.5, 0.0, 10.0, false, None, None).unwrap();
    assert_eq!(mask.name(), "线性");
    assert!(Mask::new("矩形", 0.0, 0.0, 0.5, 0.0, 101.0, false, None, None).is_err());

    let chroma = ChromaKey::new("#00FF00", 50.0, 10.0, 20.0, 5.0).unwrap();
    assert_eq!(chroma.color(), "#00FF00");
    assert!(ChromaKey::new("green", 50.0, 10.0, 20.0, 5.0).is_err());

    let background = BackgroundFilling::new("blur", 0.375, "").unwrap();
    assert_eq!(background.fill_type(), "blur");
    assert!(BackgroundFilling::new("gradient", 0.5, "").is_err());

    assert_eq!(BlendMode::new("正片叠底").unwrap().name(), "正片叠底");
    assert!(BlendMode::new(" ").is_err());
    assert!(Animation::new("Kira游动", Some(0)).is_err());
    assert!(Transition::new("3D空间", Some(0)).is_err());

    let audio = AudioEffects::new(
        Some(Fade::new(100_000, 200_000).unwrap()),
        vec![AudioEffect::new("8bit", BTreeMap::new()).unwrap()],
    )
    .unwrap();
    assert_eq!(audio.effects()[0].name(), "8bit");

    let text_style = TextStyle::new(
        Some(32.0),
        Some("#FFFFFF".to_owned()),
        Some("#000000".to_owned()),
        Some(2.0),
        Some(true),
        Some(false),
        Some(false),
        Some(1),
        None,
        Some(
            TextBackground::new("#112233", Some(1), Some(0.8), None, None, None, None, None)
                .unwrap(),
        ),
        Some(
            TextShadow::new(
                Some("#000000".to_owned()),
                Some(0.5),
                Some(45.0),
                Some(10.0),
                Some(5.0),
            )
            .unwrap(),
        ),
        Vec::new(),
        None,
        None,
    )
    .unwrap();
    assert_eq!(text_style.size(), Some(32.0));
}

#[test]
fn v1_advanced_fields_round_trip_through_domain_without_compatibility_payload() {
    let plan: jianying_cli::plan::Plan = serde_json::from_value(serde_json::json!({
        "schema": "jianying-cli-plan/v1",
        "name": "typed-advanced",
        "canvas": {"width": 1080, "height": 1920, "fps": 30},
        "tracks": [
            {"type": "video", "name": "主画面", "segments": [
                {
                    "start_us": 0, "duration_us": 1_000_000, "source": "a.mp4",
                    "keyframes": {"scale": [{"at_us": 0, "value": 1.0}, {"at_us": 500_000, "value": 1.2}]},
                    "mask": {"name": "线性", "size": 0.5, "feather": 10.0},
                    "chroma": {"color": "#00FF00", "intensity": 50.0, "shadow": 10.0, "edge_smooth": 20.0, "spill": 5.0},
                    "background_filling": {"type": "blur", "blur": 0.375, "color": ""},
                    "mix_mode": "正片叠底",
                    "animation_in": {"name": "Kira游动", "duration_us": 200_000},
                    "animation_out": {"name": "Kira游动", "duration_us": 200_000},
                    "animation_group": {"name": "三分割", "duration_us": 500_000},
                    "transition_out": {"name": "3D空间", "duration_us": 200_000}
                },
                {"start_us": 1_000_000, "duration_us": 1_000_000, "source": "b.mp4"}
            ]},
            {"type": "audio", "name": "配乐", "segments": [{
                "start_us": 0, "duration_us": 2_000_000, "source": "music.wav",
                "keyframes": {"volume": [{"at_us": 0, "value": 0.5}, {"at_us": 1_000_000, "value": 1.0}]},
                "fade": {"in_us": 100_000, "out_us": 200_000},
                "audio_effects": [{"name": "8bit", "params": {}}]
            }]},
            {"type": "text", "name": "标题", "segments": [{
                "start_us": 0, "duration_us": 1_000_000, "text": "标题AB",
                "keyframes": {"x": [{"at_us": 0, "value": 0.0}, {"at_us": 500_000, "value": 0.2}]},
                "animation_in": {"name": "冲屏位移", "duration_us": 200_000},
                "animation_out": {"name": "右上弹出", "duration_us": 200_000},
                "animation_group": {"name": "VHS", "duration_us": 500_000},
                "size": 32.0, "color": "#FFFFFF", "border_color": "#000000", "border_width": 2.0,
                "bold": true, "italic": false, "underline": false, "alignment": 1,
                "background": {"color": "#112233", "style": 1, "alpha": 0.8},
                "shadow": {"color": "#000000", "alpha": 0.5, "angle": 45.0, "distance": 10.0, "diffuse": 5.0},
                "styles": [{"range": [0, 2], "size": 36.0, "bold": true, "color": "#FF0000"}],
                "text_effect": {"effect_id": "effect-a", "resource_id": "resource-a"},
                "bubble": {"effect_id": "bubble-a", "resource_id": "bubble-resource-a"}
            }]},
            {"type": "sticker", "name": "贴纸", "segments": [{
                "start_us": 0, "duration_us": 1_000_000, "resource_id": "sticker-a",
                "keyframes": {"rotation": [{"at_us": 0, "value": 0.0}, {"at_us": 500_000, "value": 30.0}]}
            }]}
        ]
    }))
    .unwrap();
    plan.validate().unwrap();

    let project = jianying_cli::domain_compat::from_v1_plan(&plan).unwrap();
    let encoded_project = serde_json::to_string(&project).unwrap();
    let decoded_project: DraftProject = serde_json::from_str(&encoded_project).unwrap();
    assert_eq!(decoded_project, project);
    let job = jianying_cli::schema::JobV2::create(project.clone()).unwrap();
    jianying_cli::schema::JobV2::parse(&serde_json::to_string(&job).unwrap()).unwrap();
    let projected = jianying_cli::job_runner::plan_from_project(&project).unwrap();
    let original = serde_json::to_value(&plan).unwrap();
    let actual = serde_json::to_value(&projected).unwrap();

    for pointer in [
        "/tracks/0/segments/0/keyframes",
        "/tracks/0/segments/0/mask",
        "/tracks/0/segments/0/chroma",
        "/tracks/0/segments/0/background_filling",
        "/tracks/0/segments/0/mix_mode",
        "/tracks/0/segments/0/animation_in",
        "/tracks/0/segments/0/animation_out",
        "/tracks/0/segments/0/animation_group",
        "/tracks/0/segments/0/transition_out",
        "/tracks/1/segments/0/keyframes",
        "/tracks/1/segments/0/fade",
        "/tracks/1/segments/0/audio_effects",
        "/tracks/2/segments/0/keyframes",
        "/tracks/2/segments/0/animation_in",
        "/tracks/2/segments/0/animation_out",
        "/tracks/2/segments/0/animation_group",
        "/tracks/2/segments/0/size",
        "/tracks/2/segments/0/background",
        "/tracks/2/segments/0/shadow",
        "/tracks/2/segments/0/styles",
        "/tracks/2/segments/0/text_effect",
        "/tracks/2/segments/0/bubble",
        "/tracks/3/segments/0/keyframes",
    ] {
        assert_eq!(
            actual.pointer(pointer),
            original.pointer(pointer),
            "{pointer}"
        );
    }
}

#[test]
fn all_frozen_v1_fixtures_preserve_task_4_8_fields_through_domain_projection() {
    let fixture_root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/parity/scenarios");
    let mut fixtures: Vec<_> = std::fs::read_dir(fixture_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    fixtures.sort();
    assert_eq!(fixtures.len(), 55);

    let fields = [
        "crop",
        "scale",
        "x",
        "y",
        "rotation",
        "opacity",
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
        "audio_effects",
        "size",
        "color",
        "border_color",
        "border_width",
        "bold",
        "italic",
        "underline",
        "alignment",
        "font",
        "background",
        "shadow",
        "styles",
        "text_effect",
        "bubble",
    ];

    for fixture in fixtures {
        let mut scenario: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
        let mut plan_value = scenario["plan"].take();
        jianying_cli::tim::preprocess(&mut plan_value);
        let plan: jianying_cli::plan::Plan = serde_json::from_value(plan_value).unwrap();
        plan.validate().unwrap();
        let project = jianying_cli::domain_compat::from_v1_plan(&plan).unwrap();
        let projected = jianying_cli::job_runner::plan_from_project(&project).unwrap();

        assert_eq!(
            projected.tracks.len(),
            plan.tracks.len(),
            "{}",
            fixture.display()
        );
        for (track_index, (actual_track, expected_track)) in
            projected.tracks.iter().zip(&plan.tracks).enumerate()
        {
            assert_eq!(actual_track.segments.len(), expected_track.segments.len());
            for (segment_index, (actual_segment, expected_segment)) in actual_track
                .segments
                .iter()
                .zip(&expected_track.segments)
                .enumerate()
            {
                let actual = serde_json::to_value(actual_segment).unwrap();
                let expected = serde_json::to_value(expected_segment).unwrap();
                for field in fields {
                    assert_eq!(
                        actual.get(field),
                        expected.get(field),
                        "{} track {track_index} segment {segment_index} field {field}",
                        fixture.display()
                    );
                }
            }
        }
    }
}
