use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// 按 UTF-16 码元比例重算文字样式区间，并移除缩放后为空的区间。
pub fn recalculate_style_ranges(
    styles: &mut Vec<Value>,
    old_text: &str,
    new_text: &str,
) -> Result<()> {
    let old_len = i64::try_from(old_text.encode_utf16().count())?;
    let new_len = i64::try_from(new_text.encode_utf16().count())?;
    if old_len == 0 {
        if styles.is_empty() || new_len == 0 {
            styles.clear();
            return Ok(());
        }
        styles.truncate(1);
        styles[0]["range"] = json!([0, new_len]);
        return Ok(());
    }
    let mut recalculated = Vec::with_capacity(styles.len());
    for mut style in styles.drain(..) {
        let range = style["range"]
            .as_array()
            .context("text style range must be an array")?;
        if range.len() != 2 {
            bail!("text style range must contain two UTF-16 offsets");
        }
        let start = scale_ceiling(
            range[0].as_i64().context("text style start is missing")?,
            old_len,
            new_len,
        )?;
        let end = scale_ceiling(
            range[1].as_i64().context("text style end is missing")?,
            old_len,
            new_len,
        )?;
        if start < end {
            style["range"] = json!([start, end]);
            recalculated.push(style);
        }
    }
    *styles = recalculated;
    Ok(())
}

fn scale_ceiling(value: i64, old_len: i64, new_len: i64) -> Result<i64> {
    if value < 0 || value > old_len {
        bail!("text style range lies outside the old UTF-16 text length");
    }
    let product = i128::from(value) * i128::from(new_len);
    let scaled = (product + i128::from(old_len) - 1) / i128::from(old_len);
    Ok(i64::try_from(scaled)?)
}
