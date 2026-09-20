use jianying_draft::DraftTimelineWire;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct EdgeFixture {
    name: String,
    expect_reference_error: bool,
    expect_edit_error: bool,
    draft: Value,
}

#[test]
fn boundary_fixtures_preserve_wire_shape_and_enforce_declared_gates() {
    let fixtures: Vec<EdgeFixture> =
        serde_json::from_str(include_str!("fixtures/draft_edge_cases.json")).unwrap();
    assert_eq!(fixtures.len(), 7);
    for fixture in fixtures {
        let original = fixture.draft;
        let wire = DraftTimelineWire::from_value(original.clone()).unwrap();
        assert_eq!(
            wire.to_value().unwrap(),
            original,
            "{} round-trip",
            fixture.name
        );
        assert_eq!(
            wire.validate_references().is_err(),
            fixture.expect_reference_error,
            "{} reference gate",
            fixture.name
        );
        assert_eq!(
            wire.validate_edit_semantics().is_err(),
            fixture.expect_edit_error,
            "{} edit gate",
            fixture.name
        );
    }
}
