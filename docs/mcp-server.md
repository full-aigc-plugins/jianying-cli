# MCP Server：stdio、Streamable HTTP 与 SSE

`jianying mcp serve` 使用 MCP 官方 Rust SDK `rmcp 3.4.0`。SDK 负责协议版本协商、
初始化状态、JSON-RPC 消息模型、通知、取消和会话生命周期；所有 transport 共用同一个
`JianyingMcpServer` handler 与工具目录。

## 本机 stdio

默认 transport 是 stdio，适合 Codex、ZCode、Kimi 在同一台电脑上拉起子进程：

```bash
jianying mcp tools --json
jianying mcp serve --state-root .jianying-jobs
```

stdout 只承载 MCP 消息，业务日志和进度不得写入 stdout。

## Streamable HTTP

本机 HTTP 默认只监听回环地址，endpoint 为 `/mcp`：

```bash
jianying mcp serve \
  --transport streamable-http \
  --bind 127.0.0.1:8765 \
  --allowed-host 127.0.0.1:8765
```

`streamable-http` 对简单请求优先返回 `application/json`；当协议需要在最终结果前发送
通知或服务端请求时，官方 SDK 会自动切换为 `text/event-stream`。

## Pad / 局域网访问

非回环地址监听会在绑定端口前强制要求 Bearer token 和显式 Host allowlist。token 只从
环境变量读取，不作为命令行明文参数：

```bash
export JIANYING_MCP_TOKEN='替换为高强度随机值'

jianying mcp serve \
  --transport streamable-http \
  --bind 0.0.0.0:8765 \
  --allowed-host 192.168.1.20:8765 \
  --allowed-origin http://192.168.1.30:3000 \
  --token-env JIANYING_MCP_TOKEN
```

- `--allowed-host` 应填写电脑在 Pad 可达网络中的真实主机名/IP 与端口，可重复传入。
- 浏览器/网页 Pad 客户端应配置精确 `--allowed-origin`；原生客户端通常不发送 Origin。
- 客户端对 `/mcp` 的每个请求都必须发送 `Authorization: Bearer <token>`。
- 局域网明文 HTTP 不提供传输加密。跨不可信网络应在受控反向代理/VPN 后终止 TLS，
  同时保留 Host、Origin 与 token 校验；不要直接做公网端口映射。

## SSE 响应 profile

```bash
jianying mcp serve \
  --transport sse \
  --bind 127.0.0.1:8765 \
  --allowed-host 127.0.0.1:8765
```

`sse` 使用同一个现代 Streamable HTTP `/mcp` endpoint，但强制成功响应采用
`text/event-stream`。它不是已废弃的 MCP 2024-11-05 独立 HTTP+SSE transport（旧式
GET endpoint event + 独立 POST endpoint）；官方规范建议新实现使用 Streamable HTTP。

## 工具与错误语义

工具目录包含：

- `jianying_capabilities`：只读 capability manifest。
- `jianying_doctor`：只读环境探测。
- `jianying_job_run`：调用与 `jianying job run` 相同的持久 Job handler。
- `jianying_job_retry`：调用与 `jianying job retry` 相同的恢复 handler。
- `jianying_job_list`：按任务 ID 稳定排序列出持久任务。
- `jianying_job_show`：读取单个任务状态、revision、attempts 和终态原因。
- `jianying_job_cancel`：使用与 `jianying job cancel` 相同的状态机取消非终态任务。
- `jianying_job_audit`：读取任务的完整状态转换历史。

每项工具都声明 `inputSchema`、`outputSchema` 和 `annotations.readOnlyHint`。工具执行
结果同时提供 MCP `content` 与 `structuredContent`。Job 失败使用
`src/error_contract.rs` 与 CLI 共用的失败映射，因此 schema、capability、task ID 和恢复
建议不会出现两套语义。

`tests/mcp_contract.rs` 覆盖 stdio initialize/notification/tools 生命周期、持久 Job 的
list/show/cancel/audit、HTTP/SSE initialize、Bearer token、Host/Origin 拒绝路径，以及
MCP/CLI 错误字段等价。JSON-RPC `notifications/cancelled` 和 transport/session 终止仍由
官方 SDK 管理；业务任务取消通过持久 `jianying_job_cancel` 明确执行，不把连接断开等同于
已经撤销副作用。

官方参考：

- <https://github.com/modelcontextprotocol/rust-sdk>
- <https://github.com/modelcontextprotocol/rust-sdk/blob/rmcp-v3.4.0/examples/servers/src/counter_streamhttp.rs>
- <https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http>
