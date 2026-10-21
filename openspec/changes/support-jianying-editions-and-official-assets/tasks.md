## 1. 领域契约与 Schema

- [x] 1.1 为基础版、专业版、未知版型及有效/过期/未知权益编写失败优先单元测试。
- [x] 1.2 实现版型、权益状态、证据来源和最小化权益快照对象，拒绝秘密字段与无效时间窗。
- [x] 1.3 为多类官方资源收据的资源 ID、类型、层级、文件身份、条款引用、用途和限制编写正反测试。
- [x] 1.4 实现官方资源收据与不可变 SHA-256 校验，未知许可、仅预览和身份漂移 fail closed。
- [x] 1.5 发布权益快照与素材收据 JSON Schema，并覆盖未知字段、版本和向后兼容测试。

## 2. 能力解析与 CLI 命令面

- [x] 2.1 为安装身份与账号版型独立、专业安装名不得推断专业账号编写能力解析测试。
- [x] 2.2 实现基础/专业/未知路由解析器，输出所需版型、缺失能力和结构化错误。
- [x] 2.3 增加 `runtime entitlement verify` 机器命令，支持指定校验时刻和所需 capability。
- [x] 2.4 增加 `media official verify` 与 `media official register`，按资源类型重新计算下载文件或草稿资源身份并原子写入收据库。
- [x] 2.5 增加官方素材列表、查询和 Job 预检接口，输出不得包含账号秘密。
- [x] 2.6 更新命令目录、completion、错误契约和 capability manifest，未完成真实 Canary 前保持专业素材自动路由为 unavailable。
- [x] 2.7 冻结目标剪映版本的时间线语义目录与逐入口 Surface Inventory，为截图中的每个按钮实例记录语义 ID、状态、参数、首选路由、证据和回退。
- [x] 2.8 将已有 split/trim/move/remove/duplicate/track/keyframe/filter/effect/transition 等按钮语义绑定到草稿协议直接命令并增加无 GUI 黑盒测试。
- [x] 2.9 为 undo/redo、磁吸、吸附、联动、录音和时间线缩放定义 Runtime Control 状态合同；无法探测状态的控制不得标记 supported。
- [x] 2.10 为智能粗剪、文案粗剪等剪映专属动作定义 plan/preview/approve/apply/verify 状态机和失败恢复。
- [x] 2.11 增加 `runtime controls list/get/set/invoke` 命令及版本绑定的结构化失败，不公开屏幕坐标。
- [ ] 2.12 在精确版本中逐一展开页签菜单、选择工具、联动、预览轴、更多设置及右键菜单，递归登记全部子操作和可访问性名称。
  - *2026-10-21 补一条与发布限制直接相关的枚举*：官方素材面板的预览顶栏会给出**逐素材的可用性标签**：
    同一次会话里既看到 `正在预览 → 视频 · 可商用`，也看到 `正在预览 → 视频 · 不可商用`
    （截图 33 与 43）。这正是本条要的“可发布限制”的用户可见面，且它能映射到收据的 `allowed_usages`。
    证据：`docs/runbooks/evidence/windows-canary/EDITOR_SAVE_NORMALIZATION_20261021.json` 的
    `licence_labels_seen_in_the_same_panel`。
  - *2026-10-21 关于“是否必须以 beta 为准”的判断*：本条正文只要求**精确版本**，2026-09-23 那句
    `11.5.13245 / 11.6.0-beta3` 是当时的装机事实，不是要求。本机现装的 **11.6.0.14552** 是更晚的精确版本，
    已按它逐项登记（主菜单 9 / 左侧 13 页签 / 文本面板 9 组 / 导出设置 6 字段 / 时间轴工具栏图标条，
    均带截图 sha256）。所以 beta **不是**本条的阻塞项。
    另外查了 beta 通道：公开渠道只有第三方镜像站（softking、zol 等）的旧版本，没有可验证的官方
    11.6.0-beta2/beta3 安装包；客户端内的内测入口需要账号绑定，我没有凭据。故本轮未安装 beta。
    **真正阻塞**仍是专业（SVIP）权益——见上一条 2026-10-21 的账号核验记录。
  - *2026-10-21 专业账号核验未通过（阻塞记录）*：按用户回复“已用专业（SVIP）账号登录本机”，我
    `taskkill` 掉所有 JianyingPro 进程后重启客户端并截图，首页仍显示账号 **小汤圆的首部** 与
    “开通AI Ultra会员，今年享新价”推广，编辑器内仍显示“现在开通，低至1.9元/天”，**未见 SVIP 徽标**。
    证据：`docs/runbooks/evidence/windows-canary/ACCOUNT_STATE_20261021.json` 与三张截图。
    结论：本会话看不到专业权益，因此 2.12 的“可发布限制”与 editions 4.1–4.5 **一条都没有动**——
    在没有专业素材可用的情况下勾选它们就是造假。需要用户确认：是这台机器、这个账号吗？是否需要在客户端内
    退出再登录一次？
  - *2026-10-21 本机枚举进展（仍未勾）*：在已安装的 **11.6.0.14552** 上登记了主菜单 9 项、左侧面板 13 个页签、
    文本面板 9 个分组、导出设置 6 个字段（分辨率/格式/帧率/码率等实测值）与时间轴轨头，逐项带截图 sha256，
    见 `docs/runbooks/evidence/windows-canary/UI_CONTROLS_11.6.0.14552.json`。
    **仍缺**：① 本机只装了 11.6.0.14552，而本条早前记录以 **11.6.0-beta2/beta3** 的 canary 目录为准；
    ② 片段右键菜单与“页面预览台”未取到（一次截图尝试误触发了系统开始菜单）；
    ③ **可发布限制**必须用专业（SVIP）账号才能观察——当前登录是个人版，专业素材整体不可用。
  - 2026-09-23 当前安装为 `11.5.13245 / 11.6.0-beta3`，发布控件目录仍绑定 `11.5.13243 / 11.6.0-beta2`；重新采集当前版本前不得沿用旧目录宣称覆盖。7 个可展开父入口仍未枚举。
- [x] 2.13 登记播放头、时间标尺、片段拖动、片段边缘裁切、轨道拖动/重排和框选等直接手势，并为每项绑定参数、证据和回退。
- [x] 2.14 以 TDD 实现 `home folder list/create/recycle/list-recycled/restore`：精确 ID 定位、未知字段无损、真实配置运行中拒写、操作前快照、多文件原子提交与失败回滚；未观测到原生格式的非空文件夹必须失败关闭。
- [x] 2.15 以 TDD 增加 `timeline track-mute` 草稿协议命令：保持未知位与片段音量不变，精确 ID 定位、双镜像原子写入和前后回读；发布二进制及真实剪映验收前保持控件 `partial`。
- [x] 2.16 以 TDD 增加 `timeline track-rename` 草稿协议命令：精确 ID、非空名称、默认命名标记、未知字段无损、双镜像原子写入和提交后回读；发布二进制及真实剪映验收前保持界面操作 `partial`。
- [x] 2.17 以 TDD 增加 `home folder rename` 原生配置命令：精确 ID、同级重名拒绝、非空名称、关系与未知字段无损、真实配置运行中拒写、事务快照及提交后按 ID 回读；发布二进制与真实剪映冷重开前保持 `partial`。

## 3. 插件 GUI Adapter 与 Harness

- [x] 3.1 在插件独立 OpenSpec 变更中定义素材搜索、筛选、预览、下载和收据交接，不把 GUI 坐标写入 CLI。
- [x] 3.2 实现只读版型观察，明确区分应用显示名、登录状态、账号权益和单项素材下载状态。
- [x] 3.3 实现多资源域取得 Adapter，记录资源类型、资源 ID、菱形/限免等观察标记、可见条款、下载或应用结果和内容身份，不持久化 Cookie/Token。
- [x] 3.4 更新 Harness：基础版只路由本地/免费素材，专业版按有效权益与收据路由，缺失时暂停并给出可操作提示。
- [x] 3.5 为基础版、专业版、过期、未知、仅预览和下载失败建立 Fake GUI + 发布 CLI E2E。

## 4. 真实剪映与发布验收

- [ ] 4.1 使用当前专业账号取得一项明确可下载的官方素材，保存不含秘密的视觉、权益和许可证据。
  - *2026-10-21 官方素材已取得并验证（4.1 仍未勾）*：账号显示 **小汤圆的首部**，业主确认“权益已生效”。
    在编辑器里打开 官方素材，面板正常加载（左侧分类 热门/视频中国/通用/节日/…，顶部横幅
    “创意有风险，使用前请确保已获得相应版权”），素材缩略图带**蓝色 V 徽标**（VIP），预览顶栏显示
    **“正在预览 → 视频 · 可商用”**；把其中一个拖到时间轴后，剪辑以
    “流星小仙女文字片头，人物图文 00:00:03:00” 出现在轨道上并自动保存。
    证据：`docs/runbooks/evidence/windows-canary/OFFICIAL_MATERIAL_20261021.json`（6 张截图带 sha256）。
    **仍未勾**：没有拿到该素材的授权条款文本（应用只给横幅与“可商用”标签），也没有导出成片证明可发布。
- [ ] 4.2 用发布 Rust CLI 验证收据并编译真实草稿，证明篡改文件、过期权益和用途冲突均被拒绝。
  - *2026-10-21 拒绝路径已用产品命令跑通（4.2 仍未勾）*：按 `schemas/jianying-entitlement-v1` 与
    `schemas/jianying-official-resource-receipt-v1` 造了**合成**凭据/收据，逐条驱动
    `jianying media official verify … --usage … --at 1760000100`，结果：
    ① 有效专业凭据 + 匹配资源 → `status: verified`（正对照）；
    ② 过期 → `invalid entitlement evidence: expiry must be later than observation`；
    ③ basic 版验专业素材 → `runtime edition basic does not satisfy required edition professional`；
    ④ 非 active → `runtime entitlement is not active: inactive`；
    ⑤ 不允许的用途 editorial → `official asset usage is not allowed: editorial`；
    ⑥ 内容被改（sha256 不符）→ `official asset identity mismatch`；
    ⑦ 收据声明 A 而实传 B → 同样 identity mismatch；⑧ `preview_only` → 拒绝交付。
    证据（含 9 个 fixture 的 sha256 与逐条命令）：`docs/runbooks/evidence/official-resource/REFUSAL_MATRIX_20261021.json`。
    **仍缺**：真实官方素材下载、价格文件/政府授权/用户可见提示（收据契约里没有这三类字段），以及 register/list/show。
    合成 fixture 在自身 `title` 里写明“synthetic fixture (not a real asset)”，不会被误当真实素材。
- [ ] 4.3 在真实剪映完成冷重开、连续播放和原生导出，独立验证媒体流、时长、画面、音轨与来源归属。
  - *2026-10-21 漂移现在会被点名（CLI 行为改进）*：编辑器保存会换掉草稿/素材/片段 id，收据因此找不到节点，
    以前只报 `official draft resource is missing or ambiguous: … matched 0 nodes`——把责任推给调用方的
    resource id。现在 `verify_draft` 会同时读草稿摘要：如果摘要也不一致，就报新错误 `DraftContentChanged`，
    消息里带 recorded/observed 两个摘要并说明“编辑器每次保存都会重新生成 id”，恢复建议是“按当前草稿重开收据”。
    用真实数据验证：收据绑在全新 `jianying build` 草稿（`draft_content.json` 5397 B）上，去验**编辑器保存过**的
    `material-canary`（1767 B），得到 `type: draft_content_changed`。只改节点内容、id 不变时仍是 identity mismatch。
    证据：`docs/runbooks/evidence/official-resource/DRAFT_DRIFT_20261021.json`。
  - *2026-10-21 又一个前置缺口：编辑器保存会重写 CLI 草稿*。用 CLI 建的 `material-canary`（5397 B）在编辑器
    打开并保存后变成 1767 B：`fps=30.0`、`source="default"`、`extra_info.created_via`、`config` 里 12 个非空键
    被丢弃，44 个空 materials 桶与 8 个空 keyframes 桶被删；草稿 id、素材 id、片段 id **全部重新生成**。
    语义上保住了：画布 1080x1920、时长 2000000、文本内容与样式一致、片段时长一致（start=0 被省略）。
    结论：`draft_resource` 收据记录的是节点摘要与草稿摘要，**任何一次编辑器保存都会让它失效**。
    收据只能建在编辑器不会接管的草稿上，或在编辑器最后一次保存之后再建，不能跨保存。
  - *2026-10-21 加密草稿：CLI 现在会明说，而不是吐 serde 报错*。分析结果：这些文件是 Base64 信封
    （`10月21日` 3832 B → 解码 2873 B 高熵字节，仅首字节 `{` 是巧合），单字节 XOR 试不出来，是真正的加密；
    逆推 11.6 的加密方案不是我能在本轮承诺的事。
    因此做的是**让失败可执行**：`inspect_draft_resource` 识别信封（Base64 字符集且解析失败）→ 新错误
    `DraftContentEncrypted`，CLI 输出 `type: draft_content_encrypted`，消息说明“不是可读 JSON、剪映会加密
    自己建的草稿、`jianying build` 生成的仍是明文”，恢复建议指向重建草稿。用真实的 `10月21日` 验证过；
    坏掉但不是信封的文件仍按 `Io` 报（单元测试两个方向都断言）。
  - *2026-10-21 阻塞发现：编辑器会加密自己创建的草稿*。`draft_resource` 收据需要读 `draft_content.json`，
    但**编辑器新建**的草稿是加密的（`10月21日`/`10月20日`/`10月18日` 首字节为 `E2c546X0dd…`），
    而 **CLI 创建**的草稿仍是明文 JSON（`canary-template`/`canary-11.6`/`material-canary`）。
    于是素材落进了加密草稿（`10月21日`，76.2K），CLI 无法据此构造收据；两次往明文草稿
    `material-canary` 里拖素材都未生成剪辑。这是 4.2/4.3 的真实前置缺口，不是勾选问题。
- [x] 4.4 完成基础版降级环境验收，证明专业素材不被静默替换且本地素材工作流仍可运行。
  - *2026-10-21 验收通过*：`tools/official_downgrade_acceptance.py` 用**基础版权限快照**驱动发布二进制，
    连跑两次并比较，五项检查全过、两次一致：
    ① 本地素材草稿照常构建（`build` ok）；
    ② 专业收据被 `media official verify` 拒绝（退出码非 0，且没有 `data` 结果）；
    ③ 拒绝消息里点名 `required edition professional`，可行动；
    ④ `media official preflight` 同样拒绝；
    ⑤ 拒绝前后草稿目录**逐文件 sha256 完全一致**——没有被静默替换成别的素材。
    证据：`docs/runbooks/evidence/official-resource/DOWNGRADE_ACCEPTANCE_20261021.json`（含二进制 sha256）。
    诚实边界：降级环境是**喂给 CLI 的基础版凭据快照**，不是真的基础版账号；本次未驱动编辑器。
- [ ] 4.5 运行完整测试、OpenSpec strict、发布制品黑盒与三宿主验收；全部证据完成后再启用默认路由、同步并归档。
  - *2026-10-21 可执行部分已跑通（4.5 仍未勾，因为发布性与归档依赖前四项）*：
    `openspec validate --all --strict` 2 passed / 0 failed；`cargo fmt --check`、`cargo clippy -D warnings`、
    `cargo test --workspace`（123 targets / 382 passed）、`python -m unittest`（40 OK）、
    `check_provenance`（含负例自检）全部 exit 0；`parity_run.py` 跑到 55/55，且重新生成的 `PARITY_RUN.json`
    与仓库里那份**字节完全一致**；`gen_catalogs.py --check` 与上游 commit 6bb2d06 一致。
    10 个冻结产物逐个记录 sha256 且 CRLF=0。
    复现所需环境也写进证据了：ffmpeg 在 PATH、PYTHONPATH 含 tools/ 以提供 pymediainfo 适配层、
    --pyjyd-src 指向包目录、上游固定在 6bb2d06。证据：docs/runbooks/evidence/release/FINAL_GATES_20261021.json。
    **未勾原因**：发布性要求 2.12 与 4.1-4.3 完成；且没有打 tag、没有发布——那需要业主明确批准。
- [x] 4.6 用不可变发布二进制对真实首页空文件夹完成“列表 → 移入最近删除 → 回收站回读 → 恢复 → 冷重开”差分验收，并证明 GUI 条目消失但未返回活动列表时不会被误报为成功。
