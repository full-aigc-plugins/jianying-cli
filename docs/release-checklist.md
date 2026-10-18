# 发布清单（可审查、未发布）

本清单把"发布"拆成**可离线复核的步骤**。当前仓库**没有**执行第 4 步之后的任何动作：
没有打 tag、没有推送、没有上传任何制品，也没有改动 jianying-edit-plugin。

## 0. 前置

- 工作树干净（`git status --porcelain` 为空）。发布身份要求干净树：
  `release_identity` 会把脏树判为 `working_tree_unreleased`。
- `ffmpeg`/`ffprobe` 在 `PATH`（只影响运行期能力，不影响打包）。
- Python 3.11+（用 `tomllib`）。

## 1. 构建（带发布身份）

```bash
export JIANYING_BUILD_CONTRACT_STATE=committed_unreleased   # 打 tag 时才是 released
export JIANYING_BUILD_SOURCE_COMMIT="$(git rev-parse HEAD)"
unset JIANYING_BUILD_RELEASE_REF                            # 不要设成空串，二进制里要保持 null
cargo build --release --bin jianying
```

> `JIANYING_BUILD_*` 通过 `option_env!` 在编译期写入二进制；改这些变量后必须重新编译，
> 否则 `package_release.py` 会以 `binary release identity does not match` 拒绝打包。

## 2. 生成 SBOM

```bash
python tools/generate_sbom.py --output target/release/SBOM.spdx.json
```

## 3. 打包

```bash
python tools/package_release.py \
  --binary target/release/jianying.exe \
  --platform win32-x64 \
  --output-dir dist \
  --sbom target/release/SBOM.spdx.json
```

产物：

```
dist/jianying-cli-<version>-win32-x64.zip
dist/jianying-cli-<version>-win32-x64.zip.sha256
dist/jianying-cli-<version>-win32-x64.release-entry.json
```

打包器会校验：二进制版本、能力契约与 `provenance/CAPABILITIES.json` 一致、
SBOM 覆盖 `Cargo.lock` 全量包且许可证齐全、Windows runtime profile 仍为
`external_dependency/unsupported`（在真实 canary 之前不得声明支持）。

## 4. 离线复核（无需 Python）

```bash
jianying release verify dist/jianying-cli-<version>-win32-x64 >/dev/null  # 解压后的目录
# 或对解压目录：
jianying release verify <bundle-dir> --json
```

Rust 实现，逐文件复核 `manifest.json` 里的 sha256 与大小；任何改动都会被报出来
（`files_failed > 0`，退出码 1）。这也是给最终用户的自检通道。

## 5. 发布（需要显式授权）

以下动作**必须**由人确认后才执行：

1. `git tag v<version>` 并推送；
2. 用 tag 身份重建（`CONTRACT_STATE=released`、`RELEASE_REF=v<version>`）；
3. 上传 zip 与 `.sha256`；
4. 更新 jianying-edit-plugin 的 `runtime/jianying-cli.lock.json`
   （`minimumVersion` / `resolvedVersion` / `binarySha256`）并按其 AGENTS.md 要求升版本、发版。

## 6. 交给插件侧的材料

`plugin/` 目录与 `docs/integration/plugin-handoff.json` 描述了：

- 当前能力清单（`plugin/capabilities.json`，含 152+ 条，只发布 `status=supported` 的项）；
- 命令桥与检视工具的用法；
- lock 文件需要填写的字段。

## 当前状态（本轮结束时）

| 步骤 | 状态 |
|---|---|
| 1. 构建 | ✅ 本机完成（release 二进制 24.5 MB） |
| 2. SBOM | ✅ 237 个包 |
| 3. 打包 | ✅ zip + sha256 + release-entry |
| 4. 离线复核 | ✅ `jianying release verify` 通过 |
| 5. 发布 | ⛔ 未执行（等待授权） |
| 6. 插件接入 | ⛔ 未执行（等待发版授权） |
