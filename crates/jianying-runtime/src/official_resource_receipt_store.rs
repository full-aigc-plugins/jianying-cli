use crate::{OfficialAssetReceipt, RuntimeError};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 官方资源收据的本地原子存储，不包含账号凭据。
#[derive(Debug, Clone)]
pub struct OfficialResourceReceiptStore {
    root: PathBuf,
}

impl OfficialResourceReceiptStore {
    /// 创建指定根目录的收据存储。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 原子登记一份已验证收据；相同资源标识不得被静默覆盖。
    pub fn register(&self, receipt: &OfficialAssetReceipt) -> Result<PathBuf, RuntimeError> {
        receipt.validate_document()?;
        fs::create_dir_all(&self.root).map_err(|error| RuntimeError::Io {
            path: self.root.clone(),
            message: error.to_string(),
        })?;
        let path = self.path(receipt.asset_id());
        if path.exists() {
            let existing = self.show(receipt.asset_id())?;
            if existing == *receipt {
                return Ok(path);
            }
            return Err(RuntimeError::InvalidAssetReceipt(format!(
                "receipt already exists with different content: {}",
                receipt.asset_id()
            )));
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| RuntimeError::InvalidAssetReceipt(error.to_string()))?
            .as_nanos();
        let temporary = self.root.join(format!(
            ".receipt-{}-{}-{nonce}.tmp",
            std::process::id(),
            receipt_key(receipt.asset_id())
        ));
        let bytes = serde_json::to_vec_pretty(receipt)
            .map_err(|error| RuntimeError::InvalidAssetReceipt(error.to_string()))?;
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| RuntimeError::Io {
                    path: temporary.clone(),
                    message: error.to_string(),
                })?;
            file.write_all(&bytes)
                .and_then(|_| file.write_all(b"\n"))
                .and_then(|_| file.sync_all())
                .map_err(|error| RuntimeError::Io {
                    path: temporary.clone(),
                    message: error.to_string(),
                })?;
            fs::hard_link(&temporary, &path).map_err(|error| RuntimeError::Io {
                path: path.clone(),
                message: error.to_string(),
            })?;
            fs::remove_file(&temporary).map_err(|error| RuntimeError::Io {
                path: temporary.clone(),
                message: error.to_string(),
            })?;
            Ok(path.clone())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    /// 读取指定资源标识的收据并重新校验文档。
    pub fn show(&self, asset_id: &str) -> Result<OfficialAssetReceipt, RuntimeError> {
        let path = self.path(asset_id);
        let bytes = fs::read(&path).map_err(|error| RuntimeError::Io {
            path: path.clone(),
            message: error.to_string(),
        })?;
        let receipt: OfficialAssetReceipt = serde_json::from_slice(&bytes)
            .map_err(|error| RuntimeError::InvalidAssetReceipt(error.to_string()))?;
        receipt.validate_document()?;
        if receipt.asset_id() != asset_id {
            return Err(RuntimeError::InvalidAssetReceipt(
                "receipt id does not match store key".to_owned(),
            ));
        }
        Ok(receipt)
    }

    /// 按资源标识稳定排序列出所有有效收据。
    pub fn list(&self) -> Result<Vec<OfficialAssetReceipt>, RuntimeError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut receipts = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|error| RuntimeError::Io {
            path: self.root.clone(),
            message: error.to_string(),
        })? {
            let entry = entry.map_err(|error| RuntimeError::Io {
                path: self.root.clone(),
                message: error.to_string(),
            })?;
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(entry.path()).map_err(|error| RuntimeError::Io {
                path: entry.path(),
                message: error.to_string(),
            })?;
            let receipt: OfficialAssetReceipt = serde_json::from_slice(&bytes)
                .map_err(|error| RuntimeError::InvalidAssetReceipt(error.to_string()))?;
            receipt.validate_document()?;
            receipts.push(receipt);
        }
        receipts.sort_by(|left, right| left.asset_id().cmp(right.asset_id()));
        Ok(receipts)
    }

    fn path(&self, asset_id: &str) -> PathBuf {
        self.root.join(format!("{}.json", receipt_key(asset_id)))
    }
}

fn receipt_key(asset_id: &str) -> String {
    format!("{:x}", Sha256::digest(asset_id.as_bytes()))
}
