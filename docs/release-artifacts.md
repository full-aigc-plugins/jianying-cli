# Release artifacts

`.github/workflows/release-artifacts.yml` 在 tag 或手工触发时构建三个独立制品：

- `darwin-arm64`：GitHub `macos-15` arm64 runner
- `darwin-x64`：GitHub `macos-15-intel` runner
- `win32-x64`：GitHub `windows-2025` runner

流水线显式固定项目 Rust `1.98.1`（含 clippy/rustfmt）、Python 3.12 和 Node 24；来源差分、
三平台打包与 release index 生成不得依赖 runner 镜像偶然预装的解释器版本。所有 setup actions
使用完整 commit SHA 固定。

三平台 build 不会直接开始：必须先通过 `source-parity-gate`。该 job 在固定 upstream commit
上重新执行来源门禁、catalog freshness、完整 Rust workspace 的 fmt/clippy/test，以及
pyJianYingDraft 55 场景和 capcut-cli project、timeline、media-analysis、media-mutation、
material-discovery、interchange、utility/compile 七组黑盒差分；
任一 checked-in evidence 与本次运行不一致都会阻止全部发布构建。

上述三个 label 已按 GitHub 官方的
[GitHub-hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
核对：`macos-15` 为 arm64、`macos-15-intel` 为 Intel，`windows-2025` 为 x64。

每个压缩包包含二进制、`jianying-job/v2` JSON Schema、capability manifest、
许可证材料、SPDX 2.3 SBOM、逐文件 manifest 与 `SHA256SUMS`；压缩包旁另有
`.sha256`。工作流在
打包前实际执行 `--version` 与 `capabilities --json`，缺少
`capabilities`、`schema.job_v2`、`schema.compile_v1_compat`、`job.run` 或
`project.compile`、`media.asr_whisper_cpp`、`runtime.discovery` 时拒绝产出。

SBOM 生成器以完整 `cargo metadata --locked` 图为内容来源，并从 `Cargo.lock` 补入 registry
crate 的 SHA-256 checksum；打包器再次要求 SBOM 的名称/版本集合与 236 个锁定包完全一致，
逐包来源一致、所有 lock checksum 存在且相同、SPDXID 唯一、许可证声明非空。只包含根包、
遗漏依赖、伪造来源或 checksum 漂移的 SPDX JSON 即使语法有效也会在 archive 生成前失败。
`creationInfo.created` 使用 `SOURCE_DATE_EPOCH`，未显式设置时使用当前 Git 提交时间；文档命名空间
由 `Cargo.lock` 内容派生，因此相同源码提交在三个 runner 上生成逐字节一致的 SBOM。
打包器对 tar.gz 和 zip 同样使用该 epoch，固定成员排序、根路径、uid/gid、用户名、权限、ZIP
平台属性以及 gzip header 时间；源文件 mtime 或临时目录变化不得改变 archive、release-entry 和
checksum sidecar。这样已锁定 Release 在 attestation 最终一致期间重跑时，才能与既有资产逐字节
比较后继续验证，而不是要求覆盖不可变制品。

发布任务创建 Draft 后如果上传中断，`tools/release_publication_plan.py` 会把 GitHub 返回的
asset 名称、`uploaded` 状态、字节长度和 `sha256:` digest 与本次本地资产逐项比较。只有既有
资产构成逐字节相同的真子集时才允许续传，而且只上传缺失文件；完整 Draft 直接进入公开步骤。
额外资产、同名内容漂移、prerelease、错误 tag、已公开但可变的 Release 都会在公开前失败，
不会使用 `--clobber`、删除资产或污染随后不可变的发布。
同一 workflow/ref 的运行通过 GitHub Actions concurrency 保证不取消已经运行中的发布；GitHub
可能替换尚未开始的旧 pending 重跑。每个实际获准运行的发布即使初次计划
已经判定 Draft 完整，公开前仍重新获取远端 assets 并运行同一状态机，缩小检查与锁定之间的
竞态窗口。
GitHub 在 Draft 公开前尚未锁定关联 tag，因此状态机的初次和最终规划还会重新执行
`git ls-remote --tags`：lightweight tag 直接绑定 commit，annotated tag 必须使用 peeled commit，
并要求其等于发布 checkout 的完整 SHA。Draft 阶段 tag 被移动或删除时不会继续公开。

正式 tag workflow 还要求仓库配置 `RELEASE_RULESET_READ_TOKEN` secret。该 fine-grained token
只需限定本仓库的 Administration read 权限，用于读取适用的 repository/organization ruleset
完整详情；发布写入仍使用 `github.token`。若 secret 缺失或权限不足，手工预演会保留 blocked
证据，tag 发布会在构建和 Draft 创建前 fail closed。

每个平台 runner 还会在打包前执行 `tools/verify_release_runtime.py`。该检查从刚构建的目标平台
二进制验证内嵌版本/提交/ref/state、上述必需 capability、七个云端 TTS provider、审批执行参数，
并在凭据环境变量不存在时运行 MiniMax `--plan`，证明计划阶段不联网、不泄露原文且不修改草稿。
同一检查还必须从构建出的二进制执行 `media transcribe --help`，验证 executable/model 身份、
格式、语言、翻译、segment 时间戳、plan、state root 和 retry 命令面；并执行
`runtime discover --help`，确保显式搜索根和平台路径覆盖参数存在。
验证器还直接读取二进制内嵌的 `runtime.profile.windows`：在真实 Windows Runtime Profile
canary 尚未完成前，该条目必须保持 `platform=windows`、`status=external_dependency` 和
`availability=unsupported`；条目缺失或任何字段提前升级都会阻止制品打包。
`package_release.py` 自身重复执行同一 fail-closed 契约，因此不能通过跳过工作流中的 runtime
verifier 来签发能力状态提前升级的 archive。
因此未来的三平台制品不能只凭交叉编译或外部 manifest 通过发布门禁。

Release index 还固定 `repository=full-aigc-plugins/jianying-cli` 与 `releaseRef=v<version>`；
聚合器本身只接受该正典仓库，并要求 macOS archive 精确命名为
`jianying-cli-<version>-darwin-{arm64|x64}.tar.gz`、Windows archive 精确命名为
`jianying-cli-<version>-win32-x64.zip`；每个 archive 还必须具有内容精确为
`<archive SHA-256>  <archive filename>` 的相邻 `.sha256` sidecar。镜像仓库、重命名资产、
缺失或漂移的 checksum 文件都会在生成 index 前被拒绝。
插件导入时会重新验证每个下载 URL 必须精确位于该仓库和 tag 下，不能只凭“使用 HTTPS”接受
第三方镜像或跨版本制品。

每个平台同时产出 `jianying-cli-release-entry/v1` 摘要，记录 archive、二进制、
capability manifest 与 SBOM 的 SHA-256。tag 构建全部成功后，publish job 才会用
`tools/build_release_index.py` 验证三个平台条目和实际 archive，并生成
`jianying-cli-release-index/v1`。该 index 的字段与插件
`scripts/import_cli_release_index.mjs` 完全对齐；缺少任一平台、版本不等、摘要被篡改
或 capability schema 不一致都会阻止 GitHub Release 发布。

打包器还把 `release_ref`、`source_commit` 和内容状态写入每个平台的 capability
manifest 与 release-entry。脏工作树和普通分支只能生成
`working_tree_unreleased`/`committed_unreleased` canary；三平台 index 只接受干净、
与 `v<version>` tag 精确绑定且 source commit 完全相同的 `released` 条目。

本地验证当前平台的打包逻辑：

```bash
export JIANYING_BUILD_CONTRACT_STATE=working_tree_unreleased
export JIANYING_BUILD_RELEASE_REF=
export JIANYING_BUILD_SOURCE_COMMIT="$(git rev-parse HEAD)"
cargo build --release --locked
python3 tools/generate_sbom.py --output target/release/SBOM.spdx.json
python3 tools/package_release.py \
  --binary target/release/jianying \
  --platform darwin-arm64 \
  --sbom target/release/SBOM.spdx.json \
  --output-dir dist
```

`package_release.py` 会同时比较二进制内嵌 capability contract、工作树
`provenance/CAPABILITIES.json` 和本次打包身份。复用旧二进制、只改外部 manifest，或让
二进制报告的 ref/commit/state 与制品不一致都会 fail-closed。tag workflow 在编译阶段注入
`released`、tag 和 `github.sha`。包内 manifest 直接取自已验证的二进制输出（包括运行时生成的
command catalog），因此安装后的 `capabilities --json` 与包内 manifest 使用同一契约和身份。

2026-09-20 的本机 arm64 复验从当前工作树重新构建 `1.6.0`，生成包含 236 个 package 的
SPDX 2.3 SBOM，并产出 SHA-256 为
`78d07db9764fcdbc306d87371ab50fcf366a914807c7b690dfa335a95ebbd976` 的完整 archive。
二进制 SHA-256 为
`a92a23a5394477dd373e301cb5eaffcb24b7ad44c06742e6657e924167c76a0e`，内嵌身份固定源码提交
`60eae921acd4fb8161eaa69c85b31d8c2267332c`、`contract_state=working_tree_unreleased` 和空
`release_ref`。该包通过插件正式安装器逐文件校验和原子安装，并在 `PATH=/usr/bin:/bin` 下
完成 `installed_fallback` version/capability 握手；包内 SPDX SBOM SHA-256 为
`90bc8291ecd92a099ad9e0bf1b4c34f0f6b6b71e9c7b2a53c18130157abb7240`，capability manifest
SHA-256 为 `a7cf23c82c7fa0a27544794c35356f57707952c025dc91649bcb0319968b73c6`。制品还明确报告
`schema.compile_v1_compat` 与 `project.compile` 为 `supported`，并从安装后的真实二进制验证
`media tts --help` 同时公开 `local-command`、macOS/Windows 系统 TTS、loopback HTTP、
本地进程和 Edge TTS 六种执行 provider，以及小米、火山、阿里、百度、腾讯、MiniMax、智谱
七种云端 provider 及其 `--approval-id`、审批根目录和 TTS 状态根目录执行参数；火山计划还从
安装后制品验证 V3 `--resource-id`、`--uid` 与官方 SSE endpoint；`render`
命令面同时公开原生导出 `native-task` 生命周期。能力清单还报告
`media.asr_whisper_cpp=supported`，安装后二进制实际公开 `media transcribe` 的 executable/model
身份、格式、语言、翻译、segment 时间戳、plan、state root 和 retry 参数；这只证明离线命令
契约，不是实际模型转写质量证据。`runtime.discovery` 同样为 supported，安装后二进制执行
`runtime discover` 时保持 `unverified/automatic_routing=false`。制品中的 MCP 工具目录还验证
`jianying_job_list/show/cancel/retry/audit` 持久任务生命周期。制品中的
`media.audio` 与 `timeline.quantization_report` 已由安装后二进制报告为 supported；安装式黑盒
实际执行有理帧率量化命令，并验证向外对齐区间、首尾漂移与最大漂移拒绝契约。
制品中的
云端路径已接入审批、持久 ledger、单次提交至多一次网络调用、失败/不确定结果分流和草稿原子写入；本次
离线 canary 只通过 loopback mock 验证协议形状，没有调用真实付费服务。该证据只证明 arm64 发布布局可用，不是
三平台 Release 或 `released` 身份证据，因此 OpenSpec `9.5` 仍保持未完成。

2026-09-21 又在 `/tmp/jianying-cli-release-sim.UfQmjO` 建立隔离 Git 仓，把当前候选提交为
`18f2fc2632c2044987eb89f3863a6aa20507de20` 并仅在该临时仓创建本地 `v1.6.0` tag。使用该提交
重新编译的 arm64 二进制携带 `contract_state=released`、`release_ref=v1.6.0` 和相同 source
commit；严格 SBOM 包含 236 个包、225 个 registry checksum，摘要为
`b5571b09dc7538743f0ed2115ffe5faf4680e656f7fd5ea4e2241038e7d36d0c`。两次独立打包的 archive、
release-entry 和 checksum sidecar 逐字节相同，archive SHA-256 均为
`7f206eb0172c3550f766cd13814d580bddeb861ecfa5887de26f0e63e72f2deb`。插件正式 fetcher 和 bundle
installer 随后在 `PATH=/usr/bin:/bin` 下 2/2 通过，无需 Cargo。该演练证明本地 tag 身份、可复现
打包与下游消费契约闭环，但不是 GitHub Release、macOS x64、Windows x64 或真实剪映证据。

包含云端 TTS HTTP 执行器的当前源码已使用项目固定的 Rust `1.98.1` 与官方
`x86_64-apple-darwin` 标准库重新交叉构建。完整 workspace 的 x64 target
`clippy --all-targets -- -D warnings` 通过，新 capability 与量化命令接入后的 release 产物是
17,047,168 bytes 的 x86_64 Mach-O，SHA-256 为
`80c8fb9ffe126029ceed929e48b390b5e037a9260284ad57ee38833b93cb0eb4`。当前 arm64 主机没有
Rosetta，无法执行该二进制；因此该结果只计为当前源码的交叉编译和链接证据，不运行打包器、
不生成 x64 release-entry，也不替代 `macos-15-intel` runner 的 native version/capability 握手。

在本轮 TTS provider 根命令接入之前，Windows 交叉检查使用隔离安装的
`cargo-zigbuild 0.23.4`、本机 Zig 和官方
`x86_64-pc-windows-gnu` Rust 标准库，从同一工作树成功生成 10,928,640 bytes 的 PE32+ x86-64
console executable，SHA-256 为
`d5c713a35adbbcc37657f761ea4850121929e32522d312d36d9c72d62ac82548`。随后
`cargo-zigbuild test --workspace --locked --target x86_64-pc-windows-gnu --no-run` 成功链接
70 个 Windows 测试可执行文件；平台条件测试的无效 import 警告也已消除，完整 workspace 的
Windows target `clippy --all-targets -- -D warnings` 同样通过。本机没有 Wine 或 Windows
Runtime，不能执行 `--version`、capability 握手和测试，因此仍不运行打包器、不生成 Windows
release-entry，也不替代 `windows-2025` runner 的真实执行证据。当前环境已不再保留此前隔离
安装的 `cargo-zigbuild`。2026-09-21 使用现有 Zig 0.16、Homebrew LLVM `dlltool/ar` 和官方
Windows GNU Rust target 的隔离临时包装器继续复验：过滤 Rust/cc-rs 重复 target 后，全部项目与
第三方 crate（包括 ring、SQLite、Windows bindings）完成 release 编译，Rust crates 携带的
Windows import libraries 也已解析；最终只因 Zig 0.16 没有可供该 GNU 链接方式使用的 `msvcrt`
import library 而失败。完整 workspace `cargo check --all-targets` 仍通过。临时包装器已删除，
没有静默安装工具、伪造 CRT 或复用旧
Windows 摘要；后续 `windows-2025` 工作流必须从当前源码重新构建并执行。

三平台 action artifacts 汇合后的 index 生成命令：

```bash
python3 tools/build_release_index.py \
  --input-dir dist \
  --repository full-aigc-plugins/jianying-cli \
  --tag v1.6.3 \
  --output dist/jianying-cli-release-index.json
```

publish job 按 `jianying-cli-darwin-arm64`、`jianying-cli-darwin-x64`、
`jianying-cli-win32-x64` 三个精确 artifact 名称分别下载。远端预检 artifact 不进入 `dist`，
因此 index 和最终资产清单不依赖工具对未知 JSON 的隐式忽略行为。

正式发布先建立 draft Release；全部制品和 index 上传成功后才切换为公开状态。上传命令不使用
`--clobber`。同名 Release 已存在时，工作流只接受相同 tag 的稳定 immutable Release，并下载
全部资产，与当前构建逐文件比较文件名和 SHA-256；完全一致才进入恢复验证，任何差异都直接失败。
需要恢复失败的草稿发布时，应先人工审计并处理该草稿，流水线不会自行覆盖其中的文件。
工作流还会在创建草稿前调用 GitHub 的 immutable-releases 只读接口；仓库未启用原生
Immutable Releases 时直接失败。发布后再次检查 `isImmutable=true`，以 GitHub 锁定的 tag、
assets 和自动 release attestation 作为正式不可变证据，而不是仅依赖脚本约定。流水线会对
每个上传文件执行 `gh release verify-asset`；考虑证明生成的最终一致性，最多轮询 5 分钟。若
超时则发布失败，但后续重跑仍只能验证完全相同的 immutable 资产，不能上传或替换。

tag 触发时，`source-parity-gate` 会在完整 workspace 构建和来源差分之前先校验 tag 必须精确
等于根 `Cargo.toml` 的 `v<version>`，并读取仓库的 immutable-releases 设置；版本不匹配或设置
未开启都会尽早失败，避免三平台 runner 产生不能发布的制品。手工 `workflow_dispatch` 不生成
正式 Release，仍可用于 `committed_unreleased` 三平台构建验证。

同一阶段还会运行 `tools/release_preflight.py`，输出
`jianying-cli-remote-release-preflight/v1` 并始终上传 `jianying-cli-remote-preflight` artifact。
`prerequisitesReady` 和 `blockedPrerequisites` 只描述 tag 构建前必须满足的仓库 Immutable Releases
设置；`releaseComplete` 另行要求预期稳定 immutable Release 已存在，避免在创建 Release 前形成循环
依赖。手工预演即使前置条件尚未满足也会继续构建，但仍保留报告；tag 路径则在 Rust toolchain、
完整测试与三平台 runner 启动前 fail closed。publish 阶段继续二次读取设置，防止运行期间漂移。

截至 2026-09-20 的只读实时核对，`full-aigc-plugins/jianying-cli` 与
`full-aigc-plugins/jianying-edit-plugin` 的 GitHub Immutable Releases 设置均为 `enabled=false`，
`full-aigc-skills/jianying-skills` 为 `enabled=true`。本仓库设置尚未开启，因此正式 tag 工作流会按
上述预检 fail-closed；该快照不是发布授权，也不是 OpenSpec `9.5` 的完成证据。

Release 公开后，若仓库配置了 `CLI_SYNC_TOKEN`，工作流会把 tag、source commit 和 release
index SHA-256 通过 `cli-released` 事件通知 `jianying-edit-plugin`。未配置 token 时不降低
发布制品本身的完整性，插件仓每日同步任务会从正典 CLI Release 补拉同一 index。

生成工作流不等于发布完成。插件只有在取得实际 workflow artifact、核对 archive
checksum 并写入自身 CLI lock 后，才能进入 fresh-install 门禁。
