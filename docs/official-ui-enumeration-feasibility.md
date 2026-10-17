# 官方 UI 自动化枚举可行性判定

日期：2026-10-17
对象：剪映专业版 Windows 客户端 `11.6.0.14552`，安装在 `C:\Users\hnxyh\AppData\Local\JianyingPro\Apps\11.6.0.14552\`
主窗口进程：PID 38808，窗口标题「剪映专业版」

## 结论

**不可行。** 剪映的创作界面无法通过 Windows UI Automation 或 Win32 窗口枚举自动遍历。
`TIMELINE_SURFACE_INVENTORY.json` 中 7 个可展开父入口（`enumerated_parents: 0`）
无法用自动化补齐，只能人工誊录。

## 三路独立探测证据

| 探测方式 | 结果 |
|---|---|
| UIA `ControlViewWalker` | 首页 `HomePage_QMLTYPE_214`：3 元素、深度 0。创作界面 `MainWindow_QMLTYPE_621`：2 元素、深度 0 |
| UIA `RawViewWalker` | 首页 9 元素、深度 0。创作界面 2 元素、深度 0 |
| Win32 `EnumChildWindows` | 主窗口类名 `Qt622QWindowIcon`，**零子窗口**；唯一有子树的窗口是 CEF 弹层（`CefBrowserWindow` / `Chrome_RenderWidgetHostHWND`），与主 UI 无关 |

三次探测分别在**未登录首页**、**已登录首页**、**已登录创作界面**三种状态下执行，结论一致。

## 根因

技术栈为 Qt 6.2.2 + QML（`import QtQuick` 在 `VECreator.dll` 中出现 1704 次）+ CEF 弹层。
Qt Quick 把整个场景渲染进**单个 HWND**，不创建可枚举的子窗口树，也不向 UIA 暴露
可访问性树。`VECreator.dll` 中虽有 `Accessible.name: VEQmlHelper.GetAutomationIDName(this)`
这类模板级接线（35 处），但运行时并未生效到 UIA 树——静态分析的正向信号在实测中不成立。

注意：`Qt6Gui.dll` 确实包含 `QWindowsUia` 桥接代码，因此这不是「Qt6 没有 UIA 支持」，
而是**本版本剪映的 QML 组件没有把可访问性属性暴露出来**。

## 对既有工作的影响

2026-09-22 在 macOS 上人工读取 AX 树取得了 20 个 toolbar 名 + 30 个片段右键项，
该证据保留在 `jianying-edit-plugin/docs/video-harness/JIANYING_GUI_OPERATION_ACCEPTANCE_20260922.md`。
那是一次人工成果，不是自动化路径；**不能据此认为同类采集可在 Windows 复现**。

## 能力边界的重新定义

「覆盖官方剪映全部能力」不再以「枚举活体 UI 树」为判据，改以以下可机器校验的物为准：

1. **官方协议文档**（`Resources/canvas_agent/skills/jianying.hub.studio_lyra_xml_timeline_builder_v2/references/`）
   ——字段语义、单位、枚举、父子关系的权威定义，已抽取为 `docs/official-capability-reference.md`
   与 `provenance/OFFICIAL_CAPABILITY_MAP.json`。
2. **官方 agent 协议**——Selection Expression 选择器 DSL 与 `DraftEditing` 单一写入口契约。
3. **本仓 capability manifest**——`provenance/CAPABILITIES.json` 的 120 条声明及其证据。

活体 UI 树降级为**人工补充材料**，只用于校验协议文档是否覆盖了界面上的真实入口，
不作为能力清单的生成来源。

## 复现方式

探测脚本位于系统临时目录（不入库）：
`%TEMP%\uia_spike\uia_by_pid.ps1`（ControlView）、`uia_by_pid_raw.ps1`（RawView）、
`win32_children.ps1`（Win32 子窗口）。

## 官方通道的连带结论

同一环境下 `jianying-agent-cli.exe` 的只读查询可运行（任意非空
`JIANYING_AGENT_RUNTIME_TOKEN` 即返回结果），但返回 `No material source configured`——
素材源与临时凭证由宿主注入。`lyra-cli.exe` 任何调用返回
`-100015 unauthorized host process`。写入类能力（`DraftEditing`、`apply-patch`）
必须在剪映进程内完成。
