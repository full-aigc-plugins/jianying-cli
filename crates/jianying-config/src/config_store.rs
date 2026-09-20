use crate::{ConfigDocument, ConfigError, ValidationReport};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// 负责单个 profile 的隔离读取、校验与原子写入。
pub struct ConfigStore {
    root: PathBuf,
    profile: String,
    read_only: bool,
}

impl ConfigStore {
    /// 创建配置仓库；只读构造和读取不会创建目录或文件。
    pub fn new(
        root: impl Into<PathBuf>,
        profile: impl Into<String>,
        read_only: bool,
    ) -> Result<Self, ConfigError> {
        let profile = profile.into();
        if profile.is_empty()
            || !profile
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || value == b'-' || value == b'_')
        {
            return Err(ConfigError::InvalidProfile(profile));
        }
        Ok(Self {
            root: root.into(),
            profile,
            read_only,
        })
    }

    /// 返回当前 profile 的配置文件路径。
    pub fn file(&self) -> PathBuf {
        self.root
            .join("profiles")
            .join(format!("{}.json", self.profile))
    }

    /// 获取完整文档或点分隔键路径对应的值。
    pub fn get(&self, key: Option<&str>) -> Result<Value, ConfigError> {
        let document = self.load()?;
        match key {
            None => Ok(serde_json::to_value(document)?),
            Some(key) => lookup(&Value::Object(document.values), key)
                .cloned()
                .ok_or_else(|| ConfigError::KeyNotFound(key.to_owned())),
        }
    }

    /// 设置点分隔键路径；中间节点必须为对象。
    pub fn set(&self, key: &str, value: Value) -> Result<ConfigDocument, ConfigError> {
        self.ensure_writable()?;
        let parts = key_parts(key)?;
        let mut document = self.load()?;
        insert(&mut document.values, &parts, value, key)?;
        self.save(&document)?;
        Ok(document)
    }

    /// 应用 JSON Merge Patch 到 values 对象。
    pub fn patch(&self, patch: Value) -> Result<ConfigDocument, ConfigError> {
        self.ensure_writable()?;
        if !patch.is_object() {
            return Err(ConfigError::PatchMustBeObject);
        }
        let mut document = self.load()?;
        let mut values = Value::Object(document.values);
        merge_patch(&mut values, patch);
        document.values = values.as_object().cloned().unwrap_or_default();
        self.save(&document)?;
        Ok(document)
    }

    /// 删除点分隔键路径。
    pub fn unset(&self, key: &str) -> Result<ConfigDocument, ConfigError> {
        self.ensure_writable()?;
        let parts = key_parts(key)?;
        let mut document = self.load()?;
        remove(&mut document.values, &parts, key)?;
        self.save(&document)?;
        Ok(document)
    }

    /// 校验当前文件；缺失文件视为空且有效的 profile。
    pub fn validate(&self) -> Result<ValidationReport, ConfigError> {
        match self.load() {
            Ok(_) => Ok(ValidationReport {
                valid: true,
                profile: self.profile.clone(),
                issues: Vec::new(),
            }),
            Err(error) => Ok(ValidationReport {
                valid: false,
                profile: self.profile.clone(),
                issues: vec![error.to_string()],
            }),
        }
    }

    fn load(&self) -> Result<ConfigDocument, ConfigError> {
        let file = self.file();
        if !file.is_file() {
            return Ok(ConfigDocument::new(&self.profile));
        }
        let document: ConfigDocument =
            serde_json::from_slice(&std::fs::read(file).map_err(ConfigError::io)?)?;
        document.validate_for(&self.profile)?;
        Ok(document)
    }

    fn save(&self, document: &ConfigDocument) -> Result<(), ConfigError> {
        document.validate_for(&self.profile)?;
        let file = self.file();
        let parent = file.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(ConfigError::io)?;
        let temporary = file.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(document)?)
            .map_err(ConfigError::io)?;
        std::fs::rename(temporary, file).map_err(ConfigError::io)
    }

    fn ensure_writable(&self) -> Result<(), ConfigError> {
        if self.read_only {
            Err(ConfigError::ReadOnly)
        } else {
            Ok(())
        }
    }
}

fn key_parts(key: &str) -> Result<Vec<&str>, ConfigError> {
    let parts: Vec<_> = key.split('.').collect();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        Err(ConfigError::InvalidKey(key.to_owned()))
    } else {
        Ok(parts)
    }
}

fn lookup<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.')
        .try_fold(value, |current, part| current.get(part))
}

fn insert(
    object: &mut Map<String, Value>,
    parts: &[&str],
    value: Value,
    original: &str,
) -> Result<(), ConfigError> {
    if parts.len() == 1 {
        object.insert(parts[0].to_owned(), value);
        return Ok(());
    }
    let child = object
        .entry(parts[0].to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    let child = child
        .as_object_mut()
        .ok_or_else(|| ConfigError::TypeConflict(original.to_owned()))?;
    insert(child, &parts[1..], value, original)
}

fn remove(
    object: &mut Map<String, Value>,
    parts: &[&str],
    original: &str,
) -> Result<(), ConfigError> {
    if parts.len() == 1 {
        return object
            .remove(parts[0])
            .map(|_| ())
            .ok_or_else(|| ConfigError::KeyNotFound(original.to_owned()));
    }
    let child = object
        .get_mut(parts[0])
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ConfigError::KeyNotFound(original.to_owned()))?;
    remove(child, &parts[1..], original)
}

fn merge_patch(target: &mut Value, patch: Value) {
    let Value::Object(patch) = patch else {
        *target = patch;
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let target = target.as_object_mut().expect("target normalized to object");
    for (key, value) in patch {
        if value.is_null() {
            target.remove(&key);
        } else {
            merge_patch(target.entry(key).or_insert(Value::Null), value);
        }
    }
}
