use jianying_domain::DraftProject;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 作业操作针对的新建项目或已有草稿副本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectTarget {
    New {
        project: DraftProject,
    },
    Existing {
        source: PathBuf,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<PathBuf>,
    },
}
