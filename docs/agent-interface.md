# jianying 智能体接口契约（Agent Interface）

本文件是智能体调用 `jianying` 的**规范事实源**。命令面不精简、行为不隐含——
每条命令、每个字段、每个退出码都在此定义。变更需同步本文件与 `--help`。

## 1. 进程级契约

| 维度 | 规定 |
|---|---|
| 成功 | stdout **恰好一个** pretty-printed JSON 对象；退出码 0 |
| 运行错误 | stderr 一行 `error: <原因链>`（anyhow 链）；退出码 1 |
| 用法错误 | clap 用法文本到 stderr；退出码 2 |
| 幂等/安全 | `build` 拒绝已存在输出目录；`publish` 拒绝同名草稿并备份 root_meta_info；`remove` 必须 `--yes`；`verify/inspect/catalog/doctor` 纯只读 |
| 时间语义 | 一切 `*_us` 字段接受微秒整数或 `tim()` 字符串（`"1h2m3s"` / `"0.5s"` / `"500ms"` / `"1500us"` / SRT `HH:MM:SS,mmm`） |
| 能力命名 | 大小写敏感，必须命中内置目录；用 `jianying catalog` 检索可用名 |
| VIP 边界 | 目录条目默认过滤 VIP；计划 `allow_vip: true` 才放行（会员权益是用户自己的授权） |

## 2. 命令面（12 组）

### `jianying doctor`
只读环境预检。输出 `{version, plan_schema, draft_roots[], editors_running[], ffprobe?, ffmpeg?}`。
草稿根不存在时给出可操作指引；`editors_running` 非空意味着 publish 会被拒绝。

### `jianying probe <media>`
ffprobe 实测：`{path, duration_us, width, height, has_video, has_audio, is_image}`。
图片（is_image）按 pyJYD 口径计 3 小时名义时长并落 `type:"photo"`。
**规则：计划里所有 duration 必须来自 probe 实测，不许用设计值。**

### `jianying build <plan.json> --out <dir> [--srt <s.srt>] [--seed <草稿根>] [--template <草稿>]`
计划 → 草稿目录；产物 `draft_content.json` + `draft_info.json`（逐字节相同双镜像）
+ `draft_meta_info.json`（含 draft_materials 注册）+ `assets/`。构建后自动 `verify`，
失败即整体报错不产出。`--srt` 追加字幕轨；`--seed` 从草稿根最新 app 亲写草稿拷贝
schema 标记（防新版 CapCut 拒开）；`--template` 在模板时间线上叠加计划轨道。

### `jianying verify <dir>` / `jianying inspect <dir>`
结构 lint（引用完整/主轨连续/时长一致/悬空 ref）与摘要。均只读。

### `jianying publish <dir> [--root <dir>] [--force]`
拷入草稿库 + root_meta_info 注册 + `.bak` 备份。剪映/CapCut 运行中拒绝（`--force` 覆盖）；
同名拒绝（换名重建，绝不覆盖用户草稿）。

### `jianying render <dir> --out <mp4> [--scale 0.5] [--burn-captions] [--crf 28]`
ffmpeg 代理预览：平铺主视频轨 + 混音全部音频轨（含淡入淡出）+ 可选烧字幕。
**不含转场/特效/蒙版**——权威出口是剪映内导出。

### `jianying catalog [--domain <d>] [--search <子串>] [--include-vip]`
16 域能力目录检索。无参数列出各域规模；`--domain` 列条目（默认滤 VIP）；
`--search` 按显示名子串过滤。智能体写计划前应先查名。

### `jianying template <op>`（6 操作）
`inspect`（轨道+材料清单）/ `duplicate <src> <新名>` / `replace-text <dir> --track <t>
--index <i> <文本>` / `replace-material <dir> <新素材> (--name <n> | --track <t> --index <i>)`
/ `import-track <目标> <源> <轨名>`。对齐 pyJYD 模板模式；replace 后回写并保持
段 source 范围钳制。

### `jianying store <op>`
`list` / `has <名>` / `remove <名> --yes`（删除草稿目录并从 root_meta_info 注销）。

## 3. 计划契约 `jianying-cli-plan/v1`

字段手册见 [`plan-format.md`](plan-format.md)。硬校验（违规即 build 报错）：
主视频轨（如存在）必须第一轨且首段 0 起连续；同轨递增不重叠；fps∈{24,25,30,50,60}；
speed 0.1-8、volume 0-4；关键帧线性、首点 at_us=0、≥2 点、要求 speed=1 且
source_start_us=0；转场仅视频轨非末段 ≤1s；蒙版/滤镜/特效/动画名须命中目录且
参数 0-100；文本 styles 区间按 UTF-16 升序不越界。

## 4. 双源一致性（维护者）

`tools/parity_run.py`：30 场景（10 个组合动作）同一计划过 pyJYD 与 jianying-cli
语义比对，30/30 必须保持全绿。`tools/gen_catalogs.py --check`：目录与 pyJYD
metadata 一致性校验。两者都在 CI 强制执行。
