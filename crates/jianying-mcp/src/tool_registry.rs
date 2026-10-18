use crate::ToolDescriptor;
use std::collections::BTreeMap;

/// MCP 工具名到描述的确定性注册表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRegistry {
    tools: BTreeMap<String, ToolDescriptor>,
}

impl ToolRegistry {
    /// 返回 Rust runtime adapter 的标准工具集合。
    pub fn standard() -> Self {
        let descriptors = [
            ToolDescriptor::new(
                "jianying_capabilities",
                "读取结构化能力清单",
                serde_json::json!({"type":"object","additionalProperties":false}),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_doctor",
                "探测本机依赖和剪映运行时",
                serde_json::json!({"type":"object","additionalProperties":false}),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_job_run",
                "通过 CLI 共享 handler 执行 jianying-job/v2 作业",
                serde_json::json!({
                    "type":"object","required":["job"],"additionalProperties":false,
                    "properties":{"job":{"type":"string"},"out":{"type":"string"}}
                }),
                envelope_schema(),
                false,
            ),
            ToolDescriptor::new(
                "jianying_job_retry",
                "通过 CLI 共享 handler 重试持久任务",
                serde_json::json!({
                    "type":"object","required":["task_id"],"additionalProperties":false,
                    "properties":{"task_id":{"type":"string"}}
                }),
                envelope_schema(),
                false,
            ),
            ToolDescriptor::new(
                "jianying_job_list",
                "按任务 ID 稳定排序列出持久任务",
                serde_json::json!({"type":"object","additionalProperties":false}),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_job_show",
                "读取单个持久任务状态",
                task_id_schema(),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_job_cancel",
                "取消尚未进入终态的持久任务",
                task_id_schema(),
                envelope_schema(),
                false,
            ),
            ToolDescriptor::new(
                "jianying_job_audit",
                "读取持久任务的完整审计历史",
                task_id_schema(),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_render_frame",
                "导出草稿在指定时刻的单帧 PNG（无客户端抽检画面）",
                serde_json::json!({
                    "type":"object","required":["draft","at_us","out"],"additionalProperties":false,
                    "properties":{
                        "draft":{"type":"string","minLength":1},
                        "at_us":{"type":"integer","minimum":0},
                        "out":{"type":"string","minLength":1},
                        "scale":{"type":"number","default":0.5}
                    }
                }),
                envelope_schema(),
                false,
            ),
            ToolDescriptor::new(
                "jianying_draft_inspect",
                "读取草稿摘要（轨道/段数/画布/时长），等价 `project inspect`",
                serde_json::json!({
                    "type":"object","required":["draft"],"additionalProperties":false,
                    "properties":{"draft":{"type":"string","minLength":1}}
                }),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_timeline_segments",
                "列出草稿片段，可按轨道类型过滤，等价 `timeline segments`",
                serde_json::json!({
                    "type":"object","required":["draft"],"additionalProperties":false,
                    "properties":{
                        "draft":{"type":"string","minLength":1},
                        "track":{"type":"string","description":"video/audio/text/sticker/filter/effect"}
                    }
                }),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_loop",
                "评估—修复—重评闭环：跑最多 max_rounds 轮安全修复并返回每轮结果",
                serde_json::json!({
                    "type":"object","required":["draft"],"additionalProperties":false,
                    "properties":{
                        "draft":{"type":"string","minLength":1},
                        "max_rounds":{"type":"integer","minimum":0,"maximum":20,"default":3},
                        "min_score":{"type":"integer","minimum":0,"maximum":100,"default":100},
                        "deep":{"type":"boolean","default":false},
                        "policy":{"type":"string","description":"jianying-eval-policy/v1 文件路径"}
                    }
                }),
                envelope_schema(),
                false,
            ),
            ToolDescriptor::new(
                "jianying_media_probe",
                "探测媒体时长/分辨率/流/是否图片，等价 `media probe`",
                serde_json::json!({
                    "type":"object","required":["path"],"additionalProperties":false,
                    "properties":{"path":{"type":"string","minLength":1}}
                }),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_eval",
                "给草稿打分（0-100）并列出结构化问题，deep=true 时检查素材与画音质量，policy 可指定评估策略文件",
                serde_json::json!({
                    "type":"object","required":["draft"],"additionalProperties":false,
                    "properties":{
                        "draft":{"type":"string","minLength":1},
                        "deep":{"type":"boolean","default":false},
                        "policy":{"type":"string","description":"jianying-eval-policy/v1 文件路径"}
                    }
                }),
                envelope_schema(),
                true,
            ),
            ToolDescriptor::new(
                "jianying_command_list",
                "列出全部 CLI 命令路径与参数（与 `jianying commands --json` 同源），用于在 MCP 内发现能力",
                serde_json::json!({"type":"object","additionalProperties":false}),
                serde_json::json!({
                    "type":"object","required":["schema","total","commands"],
                    "properties":{"schema":{"type":"string"},"total":{"type":"integer"},"commands":{"type":"array"}}
                }),
                true,
            ),
            ToolDescriptor::new(
                "jianying_command_run",
                "按命令路径执行任意 jianying CLI 命令并返回其 JSON envelope（覆盖全部剪映功能）",
                serde_json::json!({
                    "type":"object","required":["path"],"additionalProperties":false,
                    "properties":{
                        "path":{"type":"string","minLength":1,"description":"命令路径，例如 `project build`"},
                        "args":{"type":"array","items":{"type":"string"},"description":"附加参数（不含 --json）"}
                    }
                }),
                serde_json::json!({
                    "type":"object","required":["ok","path","exit_code","response"],
                    "properties":{
                        "ok":{"type":"boolean"},"path":{"type":"string"},
                        "args":{"type":"array"},"exit_code":{"type":["integer","null"]},
                        "response":{},"stderr":{"type":"string"}
                    }
                }),
                false,
            ),
            ToolDescriptor::new(
                "jianying_command_batch",
                "按顺序执行多条命令，逐条返回结果；默认遇错即停",
                serde_json::json!({
                    "type":"object","required":["steps"],"additionalProperties":false,
                    "properties":{
                        "steps":{"type":"array","minItems":1,"items":{
                            "type":"object","required":["path"],"additionalProperties":false,
                            "properties":{
                                "path":{"type":"string","minLength":1},
                                "args":{"type":"array","items":{"type":"string"}}
                            }
                        }},
                        "stop_on_error":{"type":"boolean","default":true}
                    }
                }),
                serde_json::json!({
                    "type":"object","required":["ok","total","succeeded","failed","steps"],
                    "properties":{
                        "ok":{"type":"boolean"},"total":{"type":"integer"},
                        "succeeded":{"type":"integer"},"failed":{"type":"integer"},
                        "steps":{"type":"array"}
                    }
                }),
                false,
            ),
        ];
        let tools = descriptors
            .into_iter()
            .map(|tool| (tool.name().to_owned(), tool))
            .collect();
        Self { tools }
    }

    /// 按稳定名称查找工具。
    pub fn get(&self, name: &str) -> Option<&ToolDescriptor> {
        self.tools.get(name)
    }

    /// 按名称排序返回全部工具描述。
    pub fn list(&self) -> Vec<&ToolDescriptor> {
        self.tools.values().collect()
    }
}

fn task_id_schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object","required":["task_id"],"additionalProperties":false,
        "properties":{"task_id":{"type":"string","minLength":1}}
    })
}

fn envelope_schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object","required":["ok"],
        "properties":{"ok":{"type":"boolean"},"data":{},"error":{"type":"object"}}
    })
}
