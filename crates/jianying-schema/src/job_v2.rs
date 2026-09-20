use crate::{CompatibilityInput, ExportRequest, JobOperation, ProjectTarget, SchemaError};
use jianying_domain::{DraftProject, EditOperation};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Job v2 的稳定 schema 标识。
pub const JOB_V2_SCHEMA: &str = "jianying-job/v2";

/// Rust CLI、插件和多种智能体宿主之间共享的作业协议。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobV2 {
    schema: String,
    operation: JobOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<ProjectTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<CompatibilityInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    export: Option<ExportRequest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    operations: Vec<EditOperation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    jobs: Vec<JobV2>,
}

impl JobV2 {
    /// 创建新草稿作业。
    pub fn create(project: DraftProject) -> Result<Self, SchemaError> {
        Self::validated(
            JobOperation::Create,
            Some(ProjectTarget::New { project }),
            None,
            None,
            Vec::new(),
            Vec::new(),
        )
    }

    /// 创建携带完整旧输入语义的新草稿作业。
    pub fn create_compatible(
        project: DraftProject,
        compatibility: CompatibilityInput,
    ) -> Result<Self, SchemaError> {
        Self::validated(
            JobOperation::Create,
            Some(ProjectTarget::New { project }),
            Some(compatibility),
            None,
            Vec::new(),
            Vec::new(),
        )
    }

    /// 仅从完整、版本化兼容载荷创建草稿，无需伪造重复领域项目。
    pub fn create_from_compatibility(
        compatibility: CompatibilityInput,
    ) -> Result<Self, SchemaError> {
        Self::validated(
            JobOperation::Create,
            None,
            Some(compatibility),
            None,
            Vec::new(),
            Vec::new(),
        )
    }

    /// 创建隔离副本编辑作业；源和输出不得相同，操作列表不得为空。
    pub fn edit_existing(
        source: PathBuf,
        output: PathBuf,
        operations: Vec<EditOperation>,
    ) -> Result<Self, SchemaError> {
        Self::validated(
            JobOperation::Edit,
            Some(ProjectTarget::Existing {
                source,
                output: Some(output),
            }),
            None,
            None,
            operations,
            Vec::new(),
        )
    }

    /// 创建 inspect、verify 或 publish 已有草稿作业。
    pub fn existing(operation: JobOperation, source: PathBuf) -> Result<Self, SchemaError> {
        Self::validated(
            operation,
            Some(ProjectTarget::Existing {
                source,
                output: None,
            }),
            None,
            None,
            Vec::new(),
            Vec::new(),
        )
    }

    /// 创建已有草稿导出作业。
    pub fn export_existing(source: PathBuf, export: ExportRequest) -> Result<Self, SchemaError> {
        Self::validated(
            JobOperation::Export,
            Some(ProjectTarget::Existing {
                source,
                output: None,
            }),
            None,
            Some(export),
            Vec::new(),
            Vec::new(),
        )
    }

    /// 创建非空、不可嵌套的批处理作业。
    pub fn batch(jobs: Vec<JobV2>) -> Result<Self, SchemaError> {
        Self::validated(JobOperation::Batch, None, None, None, Vec::new(), jobs)
    }

    /// 从 JSON 解析并再次执行语义校验。
    pub fn parse(input: &str) -> Result<Self, SchemaError> {
        let job: Self = serde_json::from_str(input)?;
        job.validate()?;
        Ok(job)
    }

    /// 返回 schema 标识。
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// 返回作业操作。
    pub fn operation(&self) -> JobOperation {
        self.operation
    }

    /// 返回项目目标。
    pub fn project(&self) -> Option<&ProjectTarget> {
        self.project.as_ref()
    }

    /// 返回旧输入兼容载荷。
    pub fn compatibility(&self) -> Option<&CompatibilityInput> {
        self.compatibility.as_ref()
    }

    /// 返回导出请求。
    pub fn export(&self) -> Option<&ExportRequest> {
        self.export.as_ref()
    }

    /// 返回隔离编辑操作。
    pub fn operations(&self) -> &[EditOperation] {
        &self.operations
    }

    /// 返回批处理子作业。
    pub fn jobs(&self) -> &[JobV2] {
        &self.jobs
    }

    fn validated(
        operation: JobOperation,
        project: Option<ProjectTarget>,
        compatibility: Option<CompatibilityInput>,
        export: Option<ExportRequest>,
        operations: Vec<EditOperation>,
        jobs: Vec<JobV2>,
    ) -> Result<Self, SchemaError> {
        let job = Self {
            schema: JOB_V2_SCHEMA.to_owned(),
            operation,
            project,
            compatibility,
            export,
            operations,
            jobs,
        };
        job.validate()?;
        Ok(job)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        if self.schema != JOB_V2_SCHEMA {
            return Err(SchemaError::InvalidSchema {
                expected: JOB_V2_SCHEMA,
                actual: self.schema.clone(),
            });
        }
        if let Some(compatibility) = &self.compatibility {
            compatibility.validate()?;
            if self.operation != JobOperation::Create {
                return Err(SchemaError::InvalidField(
                    "compatibility input is only valid for create",
                ));
            }
        }
        for operation in &self.operations {
            operation
                .validate()
                .map_err(|error| SchemaError::InvalidDomain(error.to_string()))?;
        }
        match self.operation {
            JobOperation::Create if self.compatibility.is_some() => {
                if self.export.is_none() && self.operations.is_empty() && self.jobs.is_empty() {
                    if let Some(ProjectTarget::New { project }) = &self.project {
                        project
                            .validate()
                            .map_err(|error| SchemaError::InvalidDomain(error.to_string()))?;
                    } else if self.project.is_some() {
                        return Err(SchemaError::InvalidField(
                            "compatible create accepts only an optional new project",
                        ));
                    }
                    Ok(())
                } else {
                    Err(SchemaError::InvalidField(
                        "compatible create cannot carry export, operations or jobs",
                    ))
                }
            }
            JobOperation::Create => match &self.project {
                Some(ProjectTarget::New { project })
                    if self.export.is_none()
                        && self.operations.is_empty()
                        && self.jobs.is_empty() =>
                {
                    project
                        .validate()
                        .map_err(|error| SchemaError::InvalidDomain(error.to_string()))?;
                    Ok(())
                }
                _ => Err(SchemaError::InvalidField(
                    "create requires project.type=new only",
                )),
            },
            JobOperation::Edit => match &self.project {
                Some(ProjectTarget::Existing {
                    source,
                    output: Some(output),
                }) if !source.as_os_str().is_empty()
                    && !output.as_os_str().is_empty()
                    && source != output
                    && self.export.is_none()
                    && !self.operations.is_empty()
                    && self.jobs.is_empty() =>
                {
                    Ok(())
                }
                _ => Err(SchemaError::InvalidField(
                    "edit requires distinct existing source and output",
                )),
            },
            JobOperation::Inspect | JobOperation::Verify | JobOperation::Publish => {
                match &self.project {
                    Some(ProjectTarget::Existing {
                        source,
                        output: None,
                    }) if !source.as_os_str().is_empty()
                        && self.export.is_none()
                        && self.operations.is_empty()
                        && self.jobs.is_empty() =>
                    {
                        Ok(())
                    }
                    _ => Err(SchemaError::InvalidField(
                        "operation requires an existing source without output",
                    )),
                }
            }
            JobOperation::Export => match &self.project {
                Some(ProjectTarget::Existing {
                    source,
                    output: None,
                }) if !source.as_os_str().is_empty()
                    && self.export.is_some()
                    && self.operations.is_empty()
                    && self.jobs.is_empty() =>
                {
                    Ok(())
                }
                _ => Err(SchemaError::InvalidField(
                    "export requires source and export request",
                )),
            },
            JobOperation::Batch => {
                if self.project.is_some()
                    || self.compatibility.is_some()
                    || self.export.is_some()
                    || !self.operations.is_empty()
                    || self.jobs.is_empty()
                {
                    return Err(SchemaError::InvalidField(
                        "batch requires a non-empty jobs array only",
                    ));
                }
                if self
                    .jobs
                    .iter()
                    .any(|job| job.operation == JobOperation::Batch || job.validate().is_err())
                {
                    return Err(SchemaError::InvalidField(
                        "batch jobs must be valid and non-nested",
                    ));
                }
                Ok(())
            }
        }
    }
}
