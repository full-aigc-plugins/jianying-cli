use thiserror::Error;

/// 草稿 envelope 校验或补丁应用错误。
#[derive(Debug, Error)]
pub enum DraftError {
    #[error("draft root must be a JSON object")]
    RootMustBeObject,
    #[error("patch pointer must address a non-root existing value")]
    InvalidPointer,
    #[error("working draft changed outside declared patch pointers: {0}")]
    UndeclaredChange(String),
    #[error("draft wire model serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("draft materials must be a JSON object")]
    MaterialsMustBeObject,
    #[error("draft resource bucket must be an array: {0}")]
    ResourceBucketMustBeArray(String),
    #[error("draft resource in bucket {0} must be an object")]
    ResourceMustBeObject(String),
    #[error("draft resource in bucket {0} must have a non-empty id")]
    ResourceMissingId(String),
    #[error("invalid draft resource semantics in bucket {bucket}: {reason}")]
    InvalidResourceSemantic { bucket: String, reason: String },
    #[error("invalid draft keyframe resource: {0}")]
    InvalidKeyframe(String),
    #[error("duplicate material id {id} in buckets {first_bucket} and {second_bucket}")]
    DuplicateMaterialId {
        id: String,
        first_bucket: String,
        second_bucket: String,
    },
    #[error("duplicate {kind} id {id}")]
    DuplicateObjectId { kind: String, id: String },
    #[error("track {track_index} segment {segment_index} material {material_id} is missing from {bucket}")]
    MissingPrimaryMaterial {
        track_index: usize,
        segment_index: usize,
        material_id: String,
        bucket: String,
    },
    #[error(
        "track {track_index} segment {segment_index} has dangling material reference {material_id}"
    )]
    DanglingMaterialReference {
        track_index: usize,
        segment_index: usize,
        material_id: String,
    },
    #[error("draft_content.json and draft_info.json mirror mismatch")]
    TimelineMirrorMismatch,
    #[error("draft metadata field {field} does not match timeline: expected {expected}, actual {actual}")]
    MetadataMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("draft metadata path {field} is invalid: expected {expected}, actual {actual}")]
    MetadataPathMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("local material path is not registered in draft_materials: {0}")]
    MaterialNotRegistered(String),
    #[error(
        "registered material kind for {path} is incompatible: expected {expected}, actual {actual}"
    )]
    RegisteredMaterialKindMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("local material file does not exist: {0}")]
    MaterialFileMissing(String),
    #[error("local material file is outside the draft resource directory: {0}")]
    MaterialPathOutsideDraft(String),
    #[error("invalid edit semantics at {context}: {reason}")]
    InvalidEditSemantic { context: String, reason: String },
}
