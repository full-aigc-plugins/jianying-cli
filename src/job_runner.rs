use crate::draft;
use crate::plan::{Canvas, NamedIntensity, NamedParams, Plan, Segment as PlanSegment, Track};
use crate::{probe, render, store};
use anyhow::{anyhow, Result};
use jianying_domain::{DraftProject, EditOperation, Material, MaterialId, Segment, TrackKind};
use jianying_jobs::{JobRecord, JobState, SqliteJobStore};
use jianying_runtime::DraftCopySession;
use jianying_schema::{ExportKind, JobOperation, JobV2, ProjectTarget};
use serde_json::{json, Value};
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
    let record = JobRecord::new(
        task_id,
        job_path.to_path_buf(),
        output.map(Path::to_path_buf),
    )?;
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
    let base_dir = job_path.parent().unwrap_or_else(|| Path::new("."));
    run_job(&job, output, base_dir)
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
        for (index, operation) in job.operations().iter().enumerate() {
            let operation_name = serde_json::to_value(operation)?["operation"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned();
            let result = apply_edit_operation(session.copy_path(), operation)?;
            operation_results.push(json!({
                "index": index,
                "operation": operation_name,
                "result": result
            }));
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

fn apply_edit_operation(draft: &Path, operation: &EditOperation) -> Result<Value> {
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
        EditOperation::AddMaterial { .. } => Err(JobRunError::IncompatibleCapability {
            capability: "job.edit.add_material",
        }
        .into()),
        EditOperation::AddSegment { .. } => Err(JobRunError::IncompatibleCapability {
            capability: "job.edit.add_segment",
        }
        .into()),
    }
}

fn resolve_job_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
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
    let mut plan = if let Some(compatibility) = job.compatibility() {
        serde_json::from_value::<Plan>(compatibility.payload().clone())?
    } else {
        let Some(ProjectTarget::New { project }) = job.project() else {
            return Err(JobRunError::InvalidProjectShape.into());
        };
        plan_from_project(project)?
    };
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
    Ok(json!({"operation":"create", "report": report, "verification": verdict}))
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

fn plan_from_project(project: &DraftProject) -> Result<Plan> {
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
                    speed,
                    volume,
                    ..
                } => {
                    let (path, image) = local_material(project, material_id)?;
                    segment.source = Some(path);
                    segment.photo = image;
                    segment.source_start_us = source_range.start_us();
                    segment.source_duration_us = Some(source_range.duration_us());
                    segment.speed = Some(*speed);
                    segment.volume = Some(*volume);
                }
                Segment::Audio {
                    material_id,
                    source_range,
                    speed,
                    volume,
                    ..
                } => {
                    segment.source = Some(local_material(project, material_id)?.0);
                    segment.source_start_us = source_range.start_us();
                    segment.source_duration_us = Some(source_range.duration_us());
                    segment.speed = Some(*speed);
                    segment.volume = Some(*volume);
                }
                Segment::Text { text, .. } => segment.text = Some(text.clone()),
                Segment::Sticker { resource_id, .. } => {
                    segment.resource_id = Some(resource_id.clone());
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
            name: Some(source_track.id().to_owned()),
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
