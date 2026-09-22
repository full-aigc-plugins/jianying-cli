use crate::draft;
use crate::plan::{
    Animation as PlanAnimation, BackgroundFilling as PlanBackgroundFilling, Canvas,
    Chroma as PlanChroma, CropSettings as PlanCropSettings, Fade as PlanFade,
    KeyPoint as PlanKeyPoint, Mask as PlanMask, NamedIntensity, NamedParams, Plan,
    RawIds as PlanRawIds, Segment as PlanSegment, StyleRange as PlanStyleRange,
    TextBackground as PlanTextBackground, TextShadow as PlanTextShadow, Track,
    TransitionOut as PlanTransition,
};
use crate::{probe, render, store};
use anyhow::{anyhow, Result};
use jianying_domain::{
    DraftProject, EditOperation, FrameRate, Material, MaterialId, Segment, Timeline,
    Track as DomainTrack, TrackKind,
};
use jianying_jobs::{JobRecord, JobState, SqliteJobStore};
use jianying_runtime::DraftCopySession;
use jianying_schema::{ExportKind, JobOperation, JobV2, ProjectTarget};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

/// `job run` 在执行前发现的稳定契约错误。
#[derive(Debug, Error)]
pub enum JobRunError {
    #[error("operation requires --out")]
    MissingOutput,
    #[error("capability {capability} is not available")]
    IncompatibleCapability { capability: &'static str },
    #[error("job project shape does not match operation")]
    InvalidProjectShape,
    #[error("draft metadata is not a readable JSON object; lossless editing is unavailable")]
    UnsupportedDraftEncoding,
    #[error("invalid edit operation sequence: {reason}")]
    InvalidEditSequence { reason: String },
}

/// 持久任务执行失败；保留 task ID 供调用方恢复。
#[derive(Debug, Error)]
#[error("task {task_id} failed: {source:#}")]
pub struct PersistentJobError {
    pub task_id: String,
    #[source]
    pub source: anyhow::Error,
}

/// 返回任务状态目录；可由 `JIANYING_STATE_ROOT` 覆盖。
pub fn state_root(job_path: &Path, requested: Option<&Path>) -> PathBuf {
    requested
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("JIANYING_STATE_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| {
            job_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(".jianying-jobs")
        })
}

/// 新建持久任务并执行。
pub fn run_persisted(job_path: &Path, output: Option<&Path>, root: &Path) -> Result<Value> {
    let task_id = format!("jy-{}", Uuid::new_v4().simple());
    let store = SqliteJobStore::new(database_path(root));
    let output = output.map(resolve_process_path).transpose()?;
    let record = JobRecord::new(task_id, job_path.to_path_buf(), output)?;
    store.save(&record)?;
    execute_record(record, &store)
}

/// 从失败检查点重试任务。
pub fn retry_persisted(task_id: &str, root: &Path) -> Result<Value> {
    let store = SqliteJobStore::new(database_path(root));
    let mut record = store.load(task_id)?;
    record.transition(JobState::Queued, "retry requested")?;
    store.save(&record)?;
    execute_record(record, &store)
}

/// 按任务 ID 稳定排序列出持久任务。
///
/// `root` 是 CLI 与 MCP 共用的状态目录，返回值保持可直接序列化的领域记录。
pub fn list_persisted(root: &Path) -> Result<Value> {
    let store = SqliteJobStore::new(database_path(root));
    Ok(serde_json::to_value(store.list()?)?)
}

/// 读取单个持久任务。
///
/// `task_id` 必须对应现有任务；不存在时保留 JobStore 的结构化失败语义。
pub fn show_persisted(task_id: &str, root: &Path) -> Result<Value> {
    let store = SqliteJobStore::new(database_path(root));
    Ok(serde_json::to_value(store.load(task_id)?)?)
}

/// 取消尚未进入终态的持久任务。
///
/// 状态转换由 `JobRecord` 统一校验，因此 CLI 与 MCP 不会产生两套取消规则。
pub fn cancel_persisted(task_id: &str, root: &Path) -> Result<Value> {
    let store = SqliteJobStore::new(database_path(root));
    let mut record = store.load(task_id)?;
    record.transition(JobState::Cancelled, "cancel requested")?;
    store.save(&record)?;
    Ok(serde_json::to_value(record)?)
}

/// 返回任务的完整审计历史。
pub fn audit_persisted(task_id: &str, root: &Path) -> Result<Value> {
    let store = SqliteJobStore::new(database_path(root));
    let record = store.load(task_id)?;
    Ok(json!({"task_id": task_id, "history": record.history}))
}

fn execute_record(mut record: JobRecord, store: &SqliteJobStore) -> Result<Value> {
    record.transition(JobState::Running, "execution started")?;
    store.save(&record)?;
    match run_file(&record.job_path, record.output_path.as_deref()) {
        Ok(mut value) => {
            record.transition(JobState::Succeeded, "execution succeeded")?;
            store.save(&record)?;
            if let Some(object) = value.as_object_mut() {
                object.insert("task_id".to_owned(), Value::String(record.task_id.clone()));
            }
            Ok(value)
        }
        Err(source) => {
            let message = format!("{source:#}");
            record.transition(JobState::Failed, message)?;
            store.save(&record)?;
            Err(PersistentJobError {
                task_id: record.task_id,
                source,
            }
            .into())
        }
    }
}

/// 将外部稳定的状态目录映射到内部 SQLite 数据库文件。
pub fn database_path(root: &Path) -> PathBuf {
    root.join("jobs.sqlite3")
}

/// 运行 JSON Lines stdio 服务；每行请求都复用持久化 run handler。
pub fn serve(root: &Path) -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => {
                let job = request["job"].as_str().map(PathBuf::from);
                let output = request["out"].as_str().map(PathBuf::from);
                match job {
                    Some(job) => match run_persisted(&job, output.as_deref(), root) {
                        Ok(data) => json!({"ok":true,"data":data}),
                        Err(error) => {
                            let task_id = error
                                .downcast_ref::<PersistentJobError>()
                                .map(|failure| failure.task_id.clone());
                            json!({"ok":false,"error":{
                                "type":"execution_failed",
                                "message":format!("{error:#}"),
                                "details":{"task_id":task_id},
                                "recovery":task_id.map(|id| vec![format!("jianying job retry {id} --json")]).unwrap_or_default()
                            }})
                        }
                    },
                    None => json!({"ok":false,"error":{
                        "type":"invalid_request","message":"request.job is required",
                        "details":{},"recovery":[]
                    }}),
                }
            }
            Err(error) => json!({"ok":false,"error":{
                "type":"invalid_json","message":error.to_string(),"details":{},"recovery":[]
            }}),
        };
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

/// 读取并执行一个已校验的 `jianying-job/v2` 文件。
pub fn run_file(job_path: &Path, output: Option<&Path>) -> Result<Value> {
    let raw = std::fs::read_to_string(job_path)
        .map_err(|error| anyhow!("cannot read job {}: {error}", job_path.display()))?;
    let job = JobV2::parse(&raw)?;
    let base_dir = job_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let base_dir = resolve_process_path(base_dir)?;
    run_job(&job, output, &base_dir)
}

fn run_job(job: &JobV2, output: Option<&Path>, base_dir: &Path) -> Result<Value> {
    match job.operation() {
        JobOperation::Create => run_create(job, output, base_dir),
        JobOperation::Inspect => {
            let source = existing_source(job)?;
            Ok(json!({"operation":"inspect", "result": draft::inspect(source)?}))
        }
        JobOperation::Verify => {
            let source = existing_source(job)?;
            Ok(json!({"operation":"verify", "result": draft::verify(source)?}))
        }
        JobOperation::Publish => {
            let source = existing_source(job)?;
            let root = store::resolve_root(None)?;
            Ok(json!({"operation":"publish", "result": store::publish(source, &root, false)?}))
        }
        JobOperation::Export => run_export(job),
        JobOperation::Edit => run_edit(job, base_dir),
        JobOperation::Batch => run_batch(job, output, base_dir),
    }
}

fn run_edit(job: &JobV2, base_dir: &Path) -> Result<Value> {
    let Some(ProjectTarget::Existing {
        source,
        output: Some(output),
    }) = job.project()
    else {
        return Err(JobRunError::InvalidProjectShape.into());
    };
    let source = resolve_job_path(base_dir, source);
    let output = resolve_job_path(base_dir, output);
    if output.exists() {
        return Err(anyhow!("edit output {} already exists", output.display()));
    }
    // 应用可能将元数据保存为不透明编码。复制前拒绝，不能以新元数据覆盖未知字段。
    let metadata = std::fs::read(source.join("draft_meta_info.json"))?;
    if !serde_json::from_slice::<Value>(&metadata).is_ok_and(|value| value.is_object()) {
        return Err(JobRunError::UnsupportedDraftEncoding.into());
    }
    let staging = output.with_extension(format!("jianying-edit-{}.tmp", Uuid::new_v4().simple()));
    let result = (|| -> Result<Value> {
        let session = DraftCopySession::prepare(&source, &staging)?;

        // 复制得到的 wire 数据仍引用源草稿的 assets。任何 MutationPlan 首次
        // 校验前，先把这些引用改到临时副本，否则带本地媒体的合法 edit 会因
        // “素材位于草稿目录外”而在真正执行操作前失败。
        let mut staged_timeline = draft::load_timeline(session.copy_path())?;
        draft::relocate_material_paths(&mut staged_timeline, &source, session.copy_path());
        crate::template::save_timeline_as(
            session.copy_path(),
            &staged_timeline,
            session.copy_path(),
        )?;

        let mut operation_results = Vec::with_capacity(job.operations().len());
        let mut declared_materials = BTreeMap::new();
        let mut consumed_materials = BTreeSet::new();
        for (index, operation) in job.operations().iter().enumerate() {
            let operation_name = serde_json::to_value(operation)?["operation"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned();
            let result = apply_edit_operation(
                session.copy_path(),
                operation,
                base_dir,
                &mut declared_materials,
                &mut consumed_materials,
            )?;
            operation_results.push(json!({
                "index": index,
                "operation": operation_name,
                "result": result
            }));
        }
        let unused_materials = declared_materials
            .keys()
            .filter(|id| !consumed_materials.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        if !unused_materials.is_empty() {
            return Err(JobRunError::InvalidEditSequence {
                reason: format!(
                    "declared materials were not consumed by add_segment: {}",
                    unused_materials.join(", ")
                ),
            }
            .into());
        }

        // 写事务在临时副本上运行；最终提交前统一将素材路径和元数据身份
        // 改写为用户声明的输出目录，避免临时路径泄漏进成品草稿。
        let mut timeline = draft::load_timeline(session.copy_path())?;
        draft::relocate_material_paths(&mut timeline, session.copy_path(), &output);
        crate::template::save_timeline_as(session.copy_path(), &timeline, &output)?;
        draft::validate_bundle_as(session.copy_path(), &output)?;

        let isolation = session.verify()?;
        if !isolation.source_unchanged() {
            return Err(anyhow!("source draft changed during isolated edit"));
        }
        let changed_copy_files = isolation
            .changed_copy_files()
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        if changed_copy_files.is_empty() {
            return Err(anyhow!("edit operations produced no draft changes"));
        }

        std::fs::rename(&staging, &output)?;
        let verification = draft::verify(&output)?;
        if verification["ok"] != json!(true) {
            return Err(anyhow!(
                "edited draft failed verification: {}",
                verification["issues"]
            ));
        }
        Ok(json!({
            "operation": "edit",
            "source": source,
            "output": output,
            "operation_results": operation_results,
            "isolation": {
                "source_unchanged": true,
                "changed_copy_files": changed_copy_files
            },
            "verification": verification
        }))
    })();
    if result.is_err() && staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

fn apply_edit_operation(
    draft: &Path,
    operation: &EditOperation,
    base_dir: &Path,
    declared_materials: &mut BTreeMap<String, Material>,
    consumed_materials: &mut BTreeSet<String>,
) -> Result<Value> {
    match operation {
        EditOperation::ReplaceText { segment_id, text } => {
            crate::caption_ops::set_text(draft, segment_id.as_str(), text)
        }
        EditOperation::MoveSegment { segment_id, target } => crate::timeline_ops::set_segment(
            draft,
            segment_id.as_str(),
            Some(target.start_us()),
            Some(target.duration_us()),
            None,
            None,
            None,
        ),
        EditOperation::RemoveSegment { segment_id } => {
            crate::timeline_ops::remove(draft, segment_id.as_str(), true, false, false)
        }
        EditOperation::AddMaterial { material } => {
            declare_edit_material(material, base_dir, declared_materials)
        }
        EditOperation::AddSegment { track_id, segment } => {
            let result =
                add_domain_segment(draft, track_id, segment, base_dir, declared_materials)?;
            if let Some(material_id) = segment.material_id() {
                consumed_materials.insert(material_id.as_str().to_owned());
            }
            Ok(result)
        }
        EditOperation::AddTrack {
            track_id,
            name,
            kind,
            index,
        } => crate::timeline_ops::add_track_at(
            draft,
            track_id,
            kind.as_str(),
            name.as_deref().unwrap_or_else(|| kind.as_str()),
            *index,
        ),
        EditOperation::RemoveTrack { track_id } => {
            crate::timeline_ops::remove_track(draft, track_id)
        }
        EditOperation::ReorderTrack { track_id, index } => {
            crate::timeline_ops::reorder_track(draft, track_id, *index)
        }
    }
}

fn declare_edit_material(
    material: &Material,
    base_dir: &Path,
    declared_materials: &mut BTreeMap<String, Material>,
) -> Result<Value> {
    let resolved = match material {
        Material::Video { id, path } => {
            Material::video(id.clone(), resolve_job_path(base_dir, path))
        }
        Material::Audio { id, path } => {
            Material::audio(id.clone(), resolve_job_path(base_dir, path))
        }
        Material::Image { id, path } => {
            Material::image(id.clone(), resolve_job_path(base_dir, path))
        }
        Material::Font { .. } | Material::EditorResource { .. } => {
            return Err(JobRunError::IncompatibleCapability {
                capability: "job.edit.add_material",
            }
            .into());
        }
    };
    let path = match &resolved {
        Material::Video { path, .. }
        | Material::Audio { path, .. }
        | Material::Image { path, .. } => path.clone(),
        Material::Font { .. } | Material::EditorResource { .. } => unreachable!(),
    };
    if !path.is_file() {
        return Err(JobRunError::InvalidEditSequence {
            reason: format!("declared material file does not exist: {}", path.display()),
        }
        .into());
    }
    let id = resolved.id().as_str().to_owned();
    if declared_materials.insert(id.clone(), resolved).is_some() {
        return Err(JobRunError::InvalidEditSequence {
            reason: format!("duplicate material declaration: {id}"),
        }
        .into());
    }
    Ok(json!({"ok":true,"material_id":id,"path":path}))
}

fn add_domain_segment(
    draft_path: &Path,
    track_id: &str,
    segment: &Segment,
    base_dir: &Path,
    declared_materials: &BTreeMap<String, Material>,
) -> Result<Value> {
    let material = segment
        .material_id()
        .map(|id| {
            declared_materials.get(id.as_str()).cloned().ok_or_else(|| {
                JobRunError::InvalidEditSequence {
                    reason: format!(
                        "add_segment {} references undeclared material {}",
                        segment.id().as_str(),
                        id.as_str()
                    ),
                }
            })
        })
        .transpose()?;
    let timeline = draft::load_timeline(draft_path)?;
    let target_track = timeline["tracks"]
        .as_array()
        .and_then(|tracks| {
            tracks
                .iter()
                .find(|track| track["id"].as_str() == Some(track_id))
        })
        .ok_or_else(|| JobRunError::InvalidEditSequence {
            reason: format!("track not found: {track_id}"),
        })?;
    let expected_kind = segment.track_kind().as_str();
    if target_track["type"].as_str() != Some(expected_kind) {
        return Err(JobRunError::InvalidEditSequence {
            reason: format!(
                "track {track_id} has type {}, but segment {} requires {expected_kind}",
                target_track["type"].as_str().unwrap_or("unknown"),
                segment.id().as_str()
            ),
        }
        .into());
    }

    let width = u32::try_from(
        timeline["canvas_config"]["width"]
            .as_u64()
            .unwrap_or_default(),
    )?;
    let height = u32::try_from(
        timeline["canvas_config"]["height"]
            .as_u64()
            .unwrap_or_default(),
    )?;
    let fps = timeline["fps"]
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 1.0 && value.fract() == 0.0)
        .ok_or(JobRunError::IncompatibleCapability {
            capability: "timeline.rational_frame_rate",
        })? as u32;
    let temp_track_name = format!("__domain_segment_{}", Uuid::new_v4().simple());
    let domain_track = DomainTrack::new_named(
        temp_track_name.clone(),
        Some(temp_track_name.clone()),
        segment.track_kind(),
        vec![segment.clone()],
    )?;
    let project = DraftProject::new(
        "domain-segment",
        width,
        height,
        FrameRate::new(fps, 1)?,
        Timeline::new(vec![domain_track])?,
        material.into_iter().collect(),
    )?;
    let mut plan = plan_from_project(&project)?;
    plan.parent = Some(base_dir.to_path_buf());
    let transition_placeholder = plan.tracks[0].segments[0].transition_out.is_some();
    if transition_placeholder {
        // 当前 wire 编译器只允许把转场绑定到具有后继片段的片段。为保持
        // 单片段 EditOperation 的强类型语义，编译时临时追加同源后继片段，
        // 产出转场引用闭包后立即删除该占位片段和其孤儿素材。
        let mut placeholder: PlanSegment =
            serde_json::from_value(serde_json::to_value(&plan.tracks[0].segments[0])?)?;
        placeholder.start_us += placeholder.duration_us;
        placeholder.transition_out = None;
        plan.tracks[0].segments.push(placeholder);
    }
    plan.validate()?;

    let parent = resolve_process_path(draft_path.parent().unwrap_or_else(|| Path::new(".")))?;
    let temporary_draft = parent.join(format!(
        ".jianying-domain-segment-{}.tmp",
        Uuid::new_v4().simple()
    ));
    let result = (|| -> Result<Value> {
        let build = draft::build(&plan, base_dir, &temporary_draft, None, &|path| {
            probe::probe(path)
        })?;
        if transition_placeholder {
            let mut temporary_timeline = draft::load_timeline(&temporary_draft)?;
            let placeholder_id = temporary_timeline["tracks"][0]["segments"][1]["id"]
                .as_str()
                .ok_or_else(|| anyhow!("transition placeholder segment is missing"))?
                .to_owned();
            crate::timeline_ops::remove_in_timeline(
                &mut temporary_timeline,
                &placeholder_id,
                true,
                false,
            )?;
            crate::template::save_timeline(&temporary_draft, &temporary_timeline)?;
        }
        let imported =
            crate::template::import_track_at(draft_path, &temporary_draft, &temp_track_name, None)?;
        let adopted = crate::timeline_ops::adopt_imported_segment(
            draft_path,
            &temp_track_name,
            track_id,
            segment.id().as_str(),
        )?;
        Ok(json!({
            "ok":true,
            "track_id":track_id,
            "segment_id":segment.id().as_str(),
            "compiler":"draft_project_to_wire",
            "build":build,
            "import":imported,
            "adopt":adopted
        }))
    })();
    if temporary_draft.exists() {
        let _ = std::fs::remove_dir_all(&temporary_draft);
    }
    result
}

fn resolve_job_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn resolve_process_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn run_batch(job: &JobV2, output: Option<&Path>, base_dir: &Path) -> Result<Value> {
    let has_create = job
        .jobs()
        .iter()
        .any(|child| child.operation() == JobOperation::Create);
    let staging = if has_create {
        let output = output.ok_or(JobRunError::MissingOutput)?;
        if output.exists() {
            return Err(anyhow!("batch output {} already exists", output.display()));
        }
        Some(output.with_extension(format!("batch-{}.tmp", Uuid::new_v4().simple())))
    } else {
        None
    };
    if let Some(root) = &staging {
        std::fs::create_dir_all(root)?;
    }
    let mut results = Vec::with_capacity(job.jobs().len());
    for (index, child) in job.jobs().iter().enumerate() {
        let child_output = if child.operation() == JobOperation::Create {
            Some(
                staging
                    .as_ref()
                    .expect("staging exists")
                    .join(format!("{index:04}")),
            )
        } else {
            None
        };
        match run_job(child, child_output.as_deref(), base_dir) {
            Ok(result) => results.push(result),
            Err(error) => {
                if let Some(root) = &staging {
                    let _ = std::fs::remove_dir_all(root);
                }
                return Err(error);
            }
        }
    }
    if let (Some(staging), Some(output)) = (&staging, output) {
        std::fs::rename(staging, output)?;
    }
    Ok(json!({"operation":"batch","count":results.len(),"results":results}))
}

fn run_create(job: &JobV2, output: Option<&Path>, base_dir: &Path) -> Result<Value> {
    let output = output.ok_or(JobRunError::MissingOutput)?;
    if let Some(compatibility) = job
        .compatibility()
        .filter(|input| input.schema() == "capcut-cli-compile/v1")
    {
        let compile =
            crate::compile_ops::compile_payload(compatibility.payload(), base_dir, output)?;
        let verification = draft::verify(output)?;
        return Ok(json!({"operation":"create","compile":compile,"verification":verification}));
    }
    let Some(ProjectTarget::New { project }) = job.project() else {
        return Err(JobRunError::InvalidProjectShape.into());
    };

    // `jianying-cli-plan/v1` 只在输入边界转换为 DraftProject。生产执行不得再次
    // 读取 compatibility payload，否则同一 Job 会出现两个互相冲突的事实源。
    let mut plan = plan_from_project(project)?;
    plan.parent = Some(base_dir.to_path_buf());
    plan.validate()?;
    let report = draft::build(&plan, base_dir, output, None, &|path| probe::probe(path))?;
    let verdict = draft::verify(output)?;
    if verdict["ok"] != json!(true) {
        return Err(anyhow!(
            "created draft failed verification: {}",
            verdict["issues"]
        ));
    }
    Ok(json!({
        "operation":"create",
        "compiler":"draft_project_to_wire",
        "input_compatibility":job.compatibility().map(|input| input.schema()),
        "report":report,
        "verification":verdict
    }))
}

fn run_export(job: &JobV2) -> Result<Value> {
    let source = existing_source(job)?;
    let export = job.export().ok_or(JobRunError::InvalidProjectShape)?;
    match export.kind() {
        ExportKind::Proxy => {
            if export.output().exists() && !export.overwrite() {
                return Err(anyhow!(
                    "export output {} already exists and overwrite=false",
                    export.output().display()
                ));
            }
            Ok(json!({
                "operation":"export",
                "kind":"proxy",
                "result": render::render(source, export.output(), 0.5, false, 28)?
            }))
        }
        ExportKind::Native => Err(JobRunError::IncompatibleCapability {
            capability: "render.native",
        }
        .into()),
        ExportKind::DraftArchive => Err(JobRunError::IncompatibleCapability {
            capability: "store.archive",
        }
        .into()),
    }
}

fn existing_source(job: &JobV2) -> Result<&Path> {
    match job.project() {
        Some(ProjectTarget::Existing { source, .. }) => Ok(source.as_path()),
        _ => Err(JobRunError::InvalidProjectShape.into()),
    }
}

/// 将统一领域项目投影回稳定的 v1 平面计划，用于差分与当前 wire 编译器。
pub fn plan_from_project(project: &DraftProject) -> Result<Plan> {
    let frame_rate = project.frame_rate();
    if frame_rate.denominator() != 1 {
        return Err(JobRunError::IncompatibleCapability {
            capability: "timeline.rational_frame_rate",
        }
        .into());
    }
    let mut tracks = Vec::with_capacity(project.timeline().tracks().len());
    for source_track in project.timeline().tracks() {
        if source_track.kind() == TrackKind::Composite {
            return Err(JobRunError::IncompatibleCapability {
                capability: "timeline.composite",
            }
            .into());
        }
        let mut segments = Vec::with_capacity(source_track.segments().len());
        for source_segment in source_track.segments() {
            let mut segment = PlanSegment {
                start_us: source_segment.range().start_us(),
                duration_us: source_segment.range().duration_us(),
                ..PlanSegment::default()
            };
            match source_segment {
                Segment::Video {
                    material_id,
                    source_range,
                    clip,
                    transform,
                    crop,
                    keyframes,
                    mask,
                    chroma,
                    background_filling,
                    mix_mode,
                    animation_in,
                    animation_out,
                    animation_group,
                    transition_out,
                    fade,
                    ..
                } => {
                    let (path, image) = local_material(project, material_id)?;
                    segment.source = Some(path);
                    segment.photo = image;
                    segment.source_start_us = source_range.start_us();
                    segment.source_duration_us = Some(source_range.duration_us());
                    apply_clip_settings(&mut segment, clip);
                    apply_transform(&mut segment, transform);
                    segment.crop = crop.as_ref().map(|crop| PlanCropSettings {
                        upper_left_x: crop.upper_left_x(),
                        upper_left_y: crop.upper_left_y(),
                        upper_right_x: crop.upper_right_x(),
                        upper_right_y: crop.upper_right_y(),
                        lower_left_x: crop.lower_left_x(),
                        lower_left_y: crop.lower_left_y(),
                        lower_right_x: crop.lower_right_x(),
                        lower_right_y: crop.lower_right_y(),
                    });
                    apply_keyframes(&mut segment, keyframes.as_ref());
                    segment.mask = mask.as_ref().map(|mask| PlanMask {
                        name: mask.name().to_owned(),
                        center_x: mask.center_x(),
                        center_y: mask.center_y(),
                        size: mask.size(),
                        rotation: mask.rotation(),
                        feather: mask.feather(),
                        invert: mask.invert(),
                        rect_width: mask.rect_width(),
                        round_corner: mask.round_corner(),
                    });
                    segment.chroma = chroma.as_ref().map(|chroma| PlanChroma {
                        color: chroma.color().to_owned(),
                        intensity: chroma.intensity(),
                        shadow: chroma.shadow(),
                        edge_smooth: chroma.edge_smooth(),
                        spill: chroma.spill(),
                    });
                    segment.background_filling =
                        background_filling
                            .as_ref()
                            .map(|background| PlanBackgroundFilling {
                                fill_type: background.fill_type().to_owned(),
                                blur: background.blur(),
                                color: background.color().to_owned(),
                            });
                    segment.mix_mode = mix_mode.as_ref().map(|mode| mode.name().to_owned());
                    segment.animation_in = animation_in.as_ref().map(plan_animation);
                    segment.animation_out = animation_out.as_ref().map(plan_animation);
                    segment.animation_group = animation_group.as_ref().map(plan_animation);
                    segment.transition_out =
                        transition_out.as_ref().map(|transition| PlanTransition {
                            name: transition.name().to_owned(),
                            duration_us: transition.duration_us(),
                        });
                    segment.fade = fade.map(|fade| PlanFade {
                        in_us: fade.in_us(),
                        out_us: fade.out_us(),
                    });
                }
                Segment::Audio {
                    material_id,
                    source_range,
                    clip,
                    keyframes,
                    audio_effects,
                    ..
                } => {
                    segment.source = Some(local_material(project, material_id)?.0);
                    segment.source_start_us = source_range.start_us();
                    segment.source_duration_us = Some(source_range.duration_us());
                    apply_clip_settings(&mut segment, clip);
                    apply_keyframes(&mut segment, keyframes.as_ref());
                    segment.fade = audio_effects.fade().map(|fade| PlanFade {
                        in_us: fade.in_us(),
                        out_us: fade.out_us(),
                    });
                    segment.audio_effects = audio_effects
                        .effects()
                        .iter()
                        .map(|effect| NamedParams {
                            name: effect.name().to_owned(),
                            params: effect.params().clone(),
                        })
                        .collect();
                }
                Segment::Text {
                    text,
                    transform,
                    keyframes,
                    animation_in,
                    animation_out,
                    animation_group,
                    style,
                    ..
                } => {
                    segment.text = Some(text.clone());
                    apply_transform(&mut segment, transform);
                    apply_keyframes(&mut segment, keyframes.as_ref());
                    segment.animation_in = animation_in.as_ref().map(plan_animation);
                    segment.animation_out = animation_out.as_ref().map(plan_animation);
                    segment.animation_group = animation_group.as_ref().map(plan_animation);
                    apply_text_style(&mut segment, style);
                }
                Segment::Sticker {
                    resource_id,
                    transform,
                    keyframes,
                    ..
                } => {
                    segment.resource_id = Some(resource_id.clone());
                    apply_transform(&mut segment, transform);
                    apply_keyframes(&mut segment, keyframes.as_ref());
                }
                Segment::Filter {
                    resource_id,
                    intensity,
                    ..
                } => {
                    segment.filters.push(NamedIntensity {
                        name: resource_id.clone(),
                        intensity: Some(*intensity),
                    });
                    segment.intensity = Some(*intensity);
                }
                Segment::Effect {
                    resource_id,
                    parameters,
                    ..
                } => {
                    segment.effects.push(NamedParams {
                        name: resource_id.clone(),
                        params: parameters.clone(),
                    });
                    segment.params = Some(parameters.clone());
                }
                Segment::Composite { .. } => {
                    return Err(JobRunError::IncompatibleCapability {
                        capability: "timeline.composite",
                    }
                    .into());
                }
            }
            segments.push(segment);
        }
        tracks.push(Track {
            kind: source_track.kind().as_str().to_owned(),
            name: source_track.name().map(str::to_owned),
            segments,
        });
    }
    Ok(Plan {
        schema: crate::plan::SCHEMA.to_owned(),
        name: project.name().to_owned(),
        canvas: Canvas {
            width: u64::from(project.width()),
            height: u64::from(project.height()),
            fps: u64::from(frame_rate.numerator()),
        },
        tracks,
        allow_vip: false,
        parent: None,
    })
}

fn apply_clip_settings(segment: &mut PlanSegment, clip: &jianying_domain::ClipSettings) {
    segment.speed = Some(clip.speed());
    segment.volume = Some(clip.volume());
    segment.change_pitch = clip.change_pitch();
}

fn apply_transform(segment: &mut PlanSegment, transform: &jianying_domain::Transform) {
    segment.scale = transform.scale();
    segment.x = transform.x();
    segment.y = transform.y();
    segment.rotation = transform.rotation();
    segment.opacity = transform.opacity();
}

fn apply_keyframes(segment: &mut PlanSegment, keyframes: Option<&jianying_domain::Keyframes>) {
    segment.keyframes = keyframes.map(|keyframes| {
        keyframes
            .channels()
            .iter()
            .map(|(channel, points)| {
                (
                    channel.clone(),
                    points
                        .iter()
                        .map(|point| PlanKeyPoint {
                            at_us: point.at_us(),
                            value: point.value(),
                        })
                        .collect(),
                )
            })
            .collect()
    });
}

fn plan_animation(animation: &jianying_domain::Animation) -> PlanAnimation {
    PlanAnimation {
        name: animation.name().to_owned(),
        duration_us: animation.duration_us(),
    }
}

fn apply_text_style(segment: &mut PlanSegment, style: &jianying_domain::TextStyle) {
    segment.size = style.size();
    segment.color = style.color().map(str::to_owned);
    segment.border_color = style.border_color().map(str::to_owned);
    segment.border_width = style.border_width();
    segment.bold = style.bold();
    segment.italic = style.italic();
    segment.underline = style.underline();
    segment.alignment = style.alignment();
    segment.font = style.font().map(str::to_owned);
    segment.background = style.background().map(|background| PlanTextBackground {
        color: background.color().to_owned(),
        style: background.style(),
        alpha: background.alpha(),
        round_radius: background.round_radius(),
        height: background.height(),
        width: background.width(),
        horizontal_offset: background.horizontal_offset(),
        vertical_offset: background.vertical_offset(),
    });
    segment.shadow = style.shadow().map(|shadow| PlanTextShadow {
        color: shadow.color().map(str::to_owned),
        alpha: shadow.alpha(),
        angle: shadow.angle(),
        distance: shadow.distance(),
        diffuse: shadow.diffuse(),
    });
    segment.styles = style
        .styles()
        .iter()
        .map(|range| PlanStyleRange {
            range: range.range(),
            size: range.size(),
            bold: range.bold(),
            italic: range.italic(),
            underline: range.underline(),
            color: range.color().map(str::to_owned),
        })
        .collect();
    let raw_ids = |value: &jianying_domain::RawIds| PlanRawIds {
        effect_id: value.effect_id().to_owned(),
        resource_id: value.resource_id().to_owned(),
    };
    segment.text_effect = style.text_effect().map(raw_ids);
    segment.bubble = style.bubble().map(raw_ids);
}

fn local_material(project: &DraftProject, id: &MaterialId) -> Result<(PathBuf, bool)> {
    let material = project
        .materials()
        .iter()
        .find(|material| material.id() == id)
        .ok_or_else(|| anyhow!("missing material {}", id.as_str()))?;
    match material {
        Material::Video { path, .. } | Material::Audio { path, .. } => Ok((path.clone(), false)),
        Material::Image { path, .. } => Ok((path.clone(), true)),
        Material::Font { .. } | Material::EditorResource { .. } => {
            Err(anyhow!("material {} is not local media", id.as_str()))
        }
    }
}
