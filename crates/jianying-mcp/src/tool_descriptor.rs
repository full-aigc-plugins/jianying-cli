use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 可由 MCP adapter 发布的稳定工具描述。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    #[serde(rename = "outputSchema")]
    pub output_schema: Value,
    pub annotations: Value,
}

impl ToolDescriptor {
    /// 创建工具描述。
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        output_schema: Value,
        read_only: bool,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            output_schema,
            annotations: serde_json::json!({"readOnlyHint":read_only}),
        }
    }

    /// 返回稳定工具名。
    pub fn name(&self) -> &str {
        &self.name
    }
}
