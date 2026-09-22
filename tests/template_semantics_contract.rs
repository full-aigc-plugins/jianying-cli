use jianying_cli::extend_mode::ExtendMode;
use jianying_cli::replacement_timing::apply_replacement_timing;
use jianying_cli::shrink_mode::ShrinkMode;
use jianying_cli::text_style_range::recalculate_style_ranges;
use serde_json::Value;

#[test]
fn replacement_timing_and_style_ranges_match_the_fixed_python_baseline() {
    let report: Value = serde_json::from_str(include_str!(
        "../provenance/PYJYD_TEMPLATE_DIFFERENTIALS.json"
    ))
    .unwrap();
    assert_eq!(
        report["upstream_commit"],
        "c3318066d964744e2bfc66f75c71745fe8cea52a"
    );
    for case in report["timing_cases"].as_array().unwrap() {
        // Restore the common pre-operation fixture recorded by the runner.
        let index = case["segment_index"].as_u64().unwrap() as usize;
        let mut segments = if index == 0 {
            serde_json::json!([
                {"id":"target","material_id":"material","target_timerange":{"start":0,"duration":2_000_000},"source_timerange":{"start":0,"duration":2_000_000},"speed":1.0},
                {"id":"following","material_id":"material","target_timerange":{"start":2_500_000,"duration":1_000_000},"source_timerange":{"start":0,"duration":1_000_000},"speed":1.0}
            ])
        } else if case["name"] == "extend_tail" {
            serde_json::json!([
                {"id":"previous","material_id":"material","target_timerange":{"start":0,"duration":1_000_000},"source_timerange":{"start":0,"duration":1_000_000},"speed":1.0},
                {"id":"target","material_id":"material","target_timerange":{"start":2_000_000,"duration":2_000_000},"source_timerange":{"start":0,"duration":2_000_000},"speed":1.0},
                {"id":"following","material_id":"material","target_timerange":{"start":5_500_000,"duration":1_000_000},"source_timerange":{"start":0,"duration":1_000_000},"speed":1.0}
            ])
        } else {
            serde_json::json!([
                {"id":"previous","material_id":"material","target_timerange":{"start":0,"duration":1_000_000},"source_timerange":{"start":0,"duration":1_000_000},"speed":1.0},
                {"id":"target","material_id":"material","target_timerange":{"start":2_000_000,"duration":2_000_000},"source_timerange":{"start":0,"duration":2_000_000},"speed":1.0},
                {"id":"following","material_id":"material","target_timerange":{"start":4_500_000,"duration":1_000_000},"source_timerange":{"start":0,"duration":1_000_000},"speed":1.0}
            ])
        };
        let segments_array = segments.as_array_mut().unwrap();
        let shrink = ShrinkMode::parse(case["shrink_mode"].as_str().unwrap()).unwrap();
        let extend = case["extend_modes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|mode| ExtendMode::parse(mode.as_str().unwrap()).unwrap())
            .collect::<Vec<_>>();
        apply_replacement_timing(
            segments_array,
            index,
            case["source_start_us"].as_i64().unwrap(),
            case["source_duration_us"].as_i64().unwrap(),
            shrink,
            &extend,
        )
        .unwrap();
        assert_eq!(segments, case["segments"], "timing case {}", case["name"]);
    }

    for case in report["style_cases"].as_array().unwrap() {
        let mut styles = serde_json::json!([
            {"range":[0,4],"bold":false},
            {"range":[1,3],"bold":true}
        ])
        .as_array()
        .unwrap()
        .clone();
        recalculate_style_ranges(
            &mut styles,
            case["old_text"].as_str().unwrap(),
            case["new_text"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(styles, *case["styles"].as_array().unwrap());
    }
}

#[test]
fn utf16_style_recalculation_keeps_emoji_offsets_valid() {
    let mut styles = serde_json::json!([
        {"range":[0,4],"bold":false},
        {"range":[1,3],"bold":true}
    ])
    .as_array()
    .unwrap()
    .clone();
    recalculate_style_ranges(&mut styles, "A😀B", "A😀BCDEF").unwrap();
    assert_eq!(styles[0]["range"], serde_json::json!([0, 8]));
    assert_eq!(styles[1]["range"], serde_json::json!([2, 6]));
}
