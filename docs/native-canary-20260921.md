# macOS 剪映真实验收：2026-09-21

本次由用户明确授权用新建合成素材草稿执行冷重开、播放和原生导出。
仅操作 `Rust-Canary-20260921-oiVx8U`，未打开或修改其他已有草稿。

## 已通过的真实行为

- CLI 源码：`d187175`，本机 debug binary；不是新的不可变 Release。
- 应用：`/Applications/VideoFusion-macOS.app`，bundle ID `com.lemon.lvpro`。
- bundle short version：`11.5.13239`；bundle build version：`11.6.0-beta1`。
- executable SHA-256：`20cdaeb576fed9cdf85fc1858336a90291173d0ab03ccd0c225abe580ceae428`。
- 合成输入：FFmpeg testsrc2，1280×720、30 fps、6 秒，440 Hz 正弦音频；三条中英文 SRT。
- Rust `project quickstart` 创建草稿，`project verify` 返回零 issues，`store register` 登记独立名称。
- 剪映打开后，预览显示测试图和 `Rust Canary A 你好`，时间线包含三条字幕与视频。
- 实际点击播放器，时间码从 `00:00:00:10` 推进到 `00:00:05:23`，画面变化，字幕切换至 `Rust Canary C 原生导出`。
- 正常退出后，系统进程检查无 `VideoFusion-macOS` 主进程；再次启动并打开同名草稿，三条字幕和 6 秒时间线保留。
- 原生导出对话框取消云备份，导出至独立本地目录；界面明确显示“导出成功”，未点击发布。

## 导出制品验证

- 文件：`/tmp/jianying-native-canary.oiVx8U/Rust-Canary-20260921-oiVx8U.mov`。
- 容器为 MOV（不是 MP4）；H.264，1280×720，30 fps，时长 6.000000 秒。
- 音频为 AAC，44100 Hz，双声道；平均音量 -24.1 dB，峰值 -19.1 dB，排除空音轨。
- 大小：5365921 字节。
- SHA-256：`989e062a07b41650902b833cad3aed8ab3d400f7fad506df252900494f6e38bd`。
- `ffmpeg -v error -i <output> -f null -` 完整解码成功。
- 3 秒帧已人工视觉核验：测试图和 `Rust Canary B 冷重开` 正确烧录。
- 帧文件：`/tmp/jianying-native-canary.oiVx8U/native-frame-3s.png`。

界面观察保存在本次 Codex 工具记录；本记录不声称听觉验收，只记录播放进度、画面、音量表和导出音频信号。

## 真实已有草稿编辑失败

应用保存后的 `draft_meta_info.json` 为 3264 字节非 JSON 内容，不能据此断言具体加密算法。
Rust `captions list` 仍能读取原有 `draft_content.json`，但 `job run edit` 无法无损更新不透明元数据。
失败任务：`jy-1c3d6de9503046c9adb8b28d943e56c1`，输出副本未提交。

已增加复制前预检与 `unsupported_draft_encoding` 结构化错误，保留 task ID，不回显元数据、不建议重复执行同一输入。
独立合成不透明元数据测试验证源文件逐树不变；未复制应用内部实现、未绕过编码、未以重建元数据冒充无损编辑。

修复后真实重跑任务 `jy-ff6cd29f7dab498696cbdfa9ee5a475f` 返回 `unsupported_draft_encoding`；
独立遍历源目录所有文件并比较 SHA-256，得到 `sourceUnchanged=true`、`outputAbsent=true`。
8 项 `job_cli_contract` 回归测试和 OpenSpec strict 校验通过。

因此 CLI 9.3 的“创建、已有草稿编辑、冷重开、播放、原生导出”整体仍不完成。
本次证明的是通过界面控制完成的真实打开/冷重开/播放/原生导出，不能将其视为 Rust native adapter 已实现，
也不能将通用 `render.native`、`runtime.profile` 或 `captions.verify` 无条件提升为 supported。
