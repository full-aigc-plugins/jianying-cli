use crate::{
    compile_ops::CompileError, fixture_ops::FixtureError, job_runner, schema::SchemaError,
};
use anyhow::Error;
use jianying_cli_contract::ErrorEnvelope;
use serde_json::{json, Value};

/// 将内部错误转换为 CLI 与 MCP 共用的稳定失败信封。
pub fn error_envelope(error: &Error) -> ErrorEnvelope {
    let persistent = error.downcast_ref::<job_runner::PersistentJobError>();
    let effective = persistent.map_or(error, |task| &task.source);
    let task_id = persistent.map(|task| task.task_id.as_str());

    if let Some(config_error) = effective.downcast_ref::<jianying_config::ConfigError>() {
        let error_type = if matches!(config_error, jianying_config::ConfigError::ReadOnly) {
            "config_read_only"
        } else {
            "config_error"
        };
        return ErrorEnvelope::with_details(
            error_type,
            config_error.to_string(),
            json!({"profile_contract":"jianying-config/v1"}),
            Vec::new(),
        );
    }
    if let Some(FixtureError::RedactionFailed {
        bundle_dir,
        files_scanned,
        findings,
    }) = effective.downcast_ref::<FixtureError>()
    {
        return ErrorEnvelope::with_details(
            "fixture_redaction_failed",
            effective.to_string(),
            json!({
                "bundle_dir":bundle_dir,
                "files_scanned":files_scanned,
                "findings":findings
            }),
            vec!["remove or redact every reported location, then rerun `project fixture <bundle> --check`".to_owned()],
        );
    }
    if let Some(CompileError::BatchPartial { failed, results }) =
        effective.downcast_ref::<CompileError>()
    {
        return ErrorEnvelope::with_details(
            "compile_batch_partial",
            effective.to_string(),
            json!({"failed":failed,"results":results}),
            vec!["fix failed rows and rerun them with distinct draft names".to_owned()],
        );
    }
    if let Some(approval_error) = effective.downcast_ref::<jianying_jobs::ApprovalError>() {
        return ErrorEnvelope::with_details(
            "approval_denied",
            approval_error.to_string(),
            json!({}),
            vec!["request a new approval for the exact invocation context".to_owned()],
        );
    }
    if let Some(runtime_error) = effective.downcast_ref::<jianying_runtime::RuntimeError>() {
        let error_type = match runtime_error {
            jianying_runtime::RuntimeError::EntitlementExpired(_) => "entitlement_expired",
            jianying_runtime::RuntimeError::InsufficientEdition { .. }
            | jianying_runtime::RuntimeError::EntitlementNotActive(_)
            | jianying_runtime::RuntimeError::EntitlementCapabilityMissing(_) => {
                "entitlement_denied"
            }
            jianying_runtime::RuntimeError::AssetIdentityMismatch(_) => "asset_identity_mismatch",
            jianying_runtime::RuntimeError::DraftResourceUnavailable(_) => {
                "asset_resource_unavailable"
            }
            jianying_runtime::RuntimeError::ResourceIdentityKindMismatch => {
                "asset_identity_kind_mismatch"
            }
            jianying_runtime::RuntimeError::PreviewOnlyAsset
            | jianying_runtime::RuntimeError::AssetUsageNotAllowed(_) => "asset_use_denied",
            jianying_runtime::RuntimeError::UnknownControl(_) => "unknown_control",
            jianying_runtime::RuntimeError::ControlProfileMismatch { .. } => {
                "control_profile_mismatch"
            }
            jianying_runtime::RuntimeError::ControlOperationMismatch { .. } => {
                "control_operation_mismatch"
            }
            jianying_runtime::RuntimeError::ControlUnavailable { .. } => "control_unavailable",
            jianying_runtime::RuntimeError::InvalidEntitlement(_) => "invalid_entitlement",
            jianying_runtime::RuntimeError::InvalidAssetReceipt(_) => "invalid_asset_receipt",
            jianying_runtime::RuntimeError::UnsupportedProduct { .. }
            | jianying_runtime::RuntimeError::UnsupportedVersion { .. }
            | jianying_runtime::RuntimeError::UnsupportedPlatform
            | jianying_runtime::RuntimeError::UnsupportedCapability(_)
            | jianying_runtime::RuntimeError::MissingFileIdentity
            | jianying_runtime::RuntimeError::FileIdentityMismatch(_) => "unsupported_runtime",
            jianying_runtime::RuntimeError::OwnershipMismatch(_) => "runtime_ownership_mismatch",
            _ => "runtime_error",
        };
        let recovery = if error_type.starts_with("entitlement")
            || error_type.starts_with("asset_")
            || error_type == "invalid_entitlement"
            || error_type == "invalid_asset_receipt"
        {
            vec![
                "refresh entitlement evidence or select a resource with matching rights".to_owned(),
            ]
        } else {
            vec!["run `jianying runtime probe` with the exact profile and executable".to_owned()]
        };
        let mut details = json!({
            "runtime_profile_contract":"jianying-runtime-profile/v1",
            "entitlement_contract":"jianying-entitlement/v1",
            "official_resource_contract":"jianying-official-resource-receipt/v1"
        });
        if let Some(object) = details.as_object_mut() {
            match runtime_error {
                jianying_runtime::RuntimeError::EntitlementExpired(expires_at) => {
                    object.insert("expires_at_epoch_seconds".to_owned(), json!(expires_at));
                }
                jianying_runtime::RuntimeError::InsufficientEdition { required, observed } => {
                    object.insert("required_edition".to_owned(), json!(required));
                    object.insert("observed_edition".to_owned(), json!(observed));
                }
                jianying_runtime::RuntimeError::EntitlementCapabilityMissing(capability)
                | jianying_runtime::RuntimeError::UnsupportedCapability(capability) => {
                    object.insert("capability".to_owned(), json!(capability));
                }
                jianying_runtime::RuntimeError::AssetIdentityMismatch(path) => {
                    object.insert("resource".to_owned(), json!(path));
                }
                jianying_runtime::RuntimeError::DraftResourceUnavailable(resource) => {
                    object.insert("resource".to_owned(), json!(resource));
                }
                jianying_runtime::RuntimeError::AssetUsageNotAllowed(usage) => {
                    object.insert("usage".to_owned(), json!(usage));
                }
                jianying_runtime::RuntimeError::UnknownControl(control)
                | jianying_runtime::RuntimeError::ControlUnavailable { control, .. }
                | jianying_runtime::RuntimeError::ControlOperationMismatch { control, .. } => {
                    object.insert("control".to_owned(), json!(control));
                }
                jianying_runtime::RuntimeError::ControlProfileMismatch {
                    field,
                    expected,
                    observed,
                } => {
                    object.insert("field".to_owned(), json!(field));
                    object.insert("expected".to_owned(), json!(expected));
                    object.insert("observed".to_owned(), json!(observed));
                }
                _ => {}
            }
        }
        return ErrorEnvelope::with_details(
            error_type,
            runtime_error.to_string(),
            details,
            recovery,
        );
    }
    if let Some(native_error) = effective.downcast_ref::<jianying_jobs::NativeExportError>() {
        let error_type = match native_error {
            jianying_jobs::NativeExportError::Approval(_) => "approval_denied",
            jianying_jobs::NativeExportError::MissingCapability
            | jianying_jobs::NativeExportError::RuntimeEvidenceMismatch => {
                "incompatible_capability"
            }
            jianying_jobs::NativeExportError::ArtifactMissing { .. }
            | jianying_jobs::NativeExportError::ArtifactDrift { .. }
            | jianying_jobs::NativeExportError::ArtifactUnchanged { .. } => {
                "native_artifact_invalid"
            }
            _ => "native_export_error",
        };
        return ErrorEnvelope::with_details(
            error_type,
            native_error.to_string(),
            json!({"capability":"render.native"}),
            vec!["run `jianying render native ... --plan` before submitting".to_owned()],
        );
    }
    if let Some(schema_error) = effective.downcast_ref::<SchemaError>() {
        return match schema_error {
            SchemaError::InvalidSchema { expected, actual } => ErrorEnvelope::with_details(
                "unsupported_schema",
                schema_error.to_string(),
                task_details(json!({"expected":expected,"actual":actual}), task_id),
                task_recovery(vec![format!("set schema to {expected}")], task_id),
            ),
            SchemaError::InvalidCompatibilitySchema { actual } => ErrorEnvelope::with_details(
                "unsupported_schema",
                schema_error.to_string(),
                task_details(
                    json!({
                        "expected":["jianying-cli-plan/v1","capcut-cli-compile/v1"],
                        "actual":actual
                    }),
                    task_id,
                ),
                task_recovery(
                    vec!["convert the compatibility payload to jianying-cli-plan/v1 or capcut-cli-compile/v1".to_owned()],
                    task_id,
                ),
            ),
            SchemaError::InvalidField(_)
            | SchemaError::InvalidJson(_)
            | SchemaError::InvalidDomain(_) => ErrorEnvelope::with_details(
                "invalid_job",
                schema_error.to_string(),
                task_details(json!({}), task_id),
                task_recovery(
                    vec![
                        "validate the document against schemas/jianying-job-v2.schema.json"
                            .to_owned(),
                    ],
                    task_id,
                ),
            ),
        };
    }
    if let Some(job_error) = effective.downcast_ref::<job_runner::JobRunError>() {
        return match job_error {
            job_runner::JobRunError::UnsupportedDraftEncoding => ErrorEnvelope::with_details(
                "unsupported_draft_encoding",
                job_error.to_string(),
                task_details(json!({"file":"draft_meta_info.json", "required_encoding":"json_object", "source_unchanged":true}), task_id),
                vec!["use an independently verified readable draft copy; do not replace opaque metadata or retry unchanged input".to_owned()],
            ),
            job_runner::JobRunError::IncompatibleCapability { capability } => {
                ErrorEnvelope::with_details(
                    "incompatible_capability",
                    job_error.to_string(),
                    task_details(json!({"capability":capability}), task_id),
                    task_recovery(
                        vec!["inspect provenance/CAPABILITIES.json before routing".to_owned()],
                        task_id,
                    ),
                )
            }
            job_runner::JobRunError::MissingOutput => ErrorEnvelope::with_details(
                "invalid_input",
                job_error.to_string(),
                task_details(json!({"argument":"--out"}), task_id),
                task_recovery(
                    vec!["provide --out for create operations".to_owned()],
                    task_id,
                ),
            ),
            job_runner::JobRunError::InvalidProjectShape => ErrorEnvelope::with_details(
                "invalid_job",
                job_error.to_string(),
                task_details(json!({}), task_id),
                task_recovery(
                    vec!["validate the operation and project target combination".to_owned()],
                    task_id,
                ),
            ),
            job_runner::JobRunError::InvalidEditSequence { reason } => {
                ErrorEnvelope::with_details(
                    "invalid_job",
                    job_error.to_string(),
                    task_details(json!({"reason":reason}), task_id),
                    task_recovery(
                        vec!["order add_material and add_track before add_segment, then consume every declared material".to_owned()],
                        task_id,
                    ),
                )
            }
        };
    }
    ErrorEnvelope::with_details(
        "execution_failed",
        format!("{effective:#}"),
        task_details(json!({}), task_id),
        task_recovery(Vec::new(), task_id),
    )
}

fn task_details(mut details: Value, task_id: Option<&str>) -> Value {
    if let (Some(object), Some(task_id)) = (details.as_object_mut(), task_id) {
        object.insert("task_id".to_owned(), Value::String(task_id.to_owned()));
    }
    details
}

fn task_recovery(mut recovery: Vec<String>, task_id: Option<&str>) -> Vec<String> {
    if let Some(task_id) = task_id {
        recovery.push(format!("jianying job retry {task_id} --json"));
    }
    recovery
}
