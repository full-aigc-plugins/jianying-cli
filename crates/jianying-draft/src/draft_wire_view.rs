use serde_json::Value;

/// 从原始草稿 JSON 投影出的只读、容错视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftWireView {
    id: Option<String>,
    track_count: usize,
    material_count: usize,
}

impl DraftWireView {
    pub(crate) fn from_value(value: &Value) -> Self {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let track_count = value
            .get("tracks")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let material_count = value
            .get("materials")
            .and_then(Value::as_object)
            .map(|groups| {
                groups
                    .values()
                    .filter_map(Value::as_array)
                    .map(Vec::len)
                    .sum()
            })
            .unwrap_or(0);
        Self {
            id,
            track_count,
            material_count,
        }
    }

    /// 返回草稿标识；旧格式无标识时返回 `None`。
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// 返回顶层轨道数。
    pub fn track_count(&self) -> usize {
        self.track_count
    }

    /// 返回所有已识别素材数组的元素总数。
    pub fn material_count(&self) -> usize {
        self.material_count
    }
}
