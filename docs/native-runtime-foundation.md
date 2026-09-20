# 本机运行时基础契约

本文件记录自主实现的 `jianying-runtime` 基础层。它只使用本项目编写的合成文件、临时目录和
测试进程，不读取或派生 `jianying-headless` 的源码、测试、蓝图、资源、fixture 或实现常量。

## Runtime Profile 与探测

`RuntimeProfile` 对一个精确支持组合绑定：

- profile id、产品名、精确版本和平台；
- 可执行文件路径、字节长度和 SHA-256；
- 编辑器进程名集合；
- 获准访问的草稿根目录；
- 可用 capability 集合。

`RuntimeProfile::probe` 对产品、版本、平台、文件身份、草稿根、素材和请求 capability 逐项检查。
任何未知版本、内容身份变化、未批准根目录、缺失素材或未声明 capability 都返回结构化错误，
不会降级为“尽力执行”。进程观察与 profile 中的精确进程名匹配，写入门禁在编辑器运行时返回
结构化恢复 argv：`jianying runtime stop --profile <id>`。

`jianying runtime discover` 只读扫描平台已知安装位置和草稿根：发现安装时计算 executable
SHA-256/长度，并读取可得的 macOS bundle 版本；草稿根存在而应用缺失时明确返回
`drafts_without_editor`。发现结果固定为 `support_status=unverified`、
`automatic_routing=false`，不能替代精确 Runtime Profile 或真实宿主 canary。

当前探测层是可复用的判定内核；支持 profile 仍需 8.9 与 9.3 的合法宿主 canary，因此
capability manifest 不把真实剪映 runtime 控制宣称为 supported。

## 已有草稿隔离编辑

`DraftCopySession::prepare` 只接受目录源和不存在的目标路径。它在复制前对所有常规文件记录
相对路径、字节长度和 SHA-256，拒绝符号链接，也拒绝把副本放进源目录。编辑器只能通过
`copy_path()` 获得副本路径。

`verify()` 再次扫描源与副本，返回：

- 复制前源目录的逐文件快照；
- 编辑后源目录的逐文件快照；
- 编辑后副本的逐文件快照；
- 副本中增加、删除或内容变化的相对文件路径。

测试使用本项目生成的多轨 JSON 和二进制素材，修改副本文字与音量后，逐字节证明源目录未变。

## 安全进程控制

`LocalRuntimeAdapter` 只接受绝对可执行路径，并在启动前重新计算内容身份。启动使用
`Command::new(...).args(...)` 的结构化 argv，不经过 shell。实例只持有自己启动的 `Child`，
`stop` 不按进程名全局终止进程；重复 start、重复 stop 都显式失败。adapter 被释放时会尽力终止
仍由它持有的子进程，避免测试或调用方异常退出遗留孤儿进程。

## 当前证据边界

- `runtime_profile_probe.rs`：产品/版本/平台/身份/权限/草稿根/素材/capability 与运行中写入拒绝。
- `runtime_discovery.rs`：应用安装去重、版本/文件身份、草稿根和禁止自动路由；CLI 黑盒还覆盖
  安装存在与 `drafts_without_editor` 两种状态。
- `isolated_draft_copy.rs`：独立副本、源文件全量不变证明、目标路径安全约束。
- `local_runtime_adapter.rs`：真实本机合成进程的 start/status/stop、重复操作失败。

这些证据完成基础实现，不等同于真实剪映兼容、冷重开、播放或原生导出验收。
