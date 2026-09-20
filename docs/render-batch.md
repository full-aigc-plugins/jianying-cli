# 代理渲染与批处理

`render proxy` 和 `render batch` 生成 FFmpeg 代理预览，不冒充剪映原生导出。原生导出仍需
具名 Runtime Profile、精确审批和真实宿主验证。

单草稿代理预览：

```bash
jianying render proxy ./draft --out preview.mp4 \
  --scale 0.5 --burn-captions --crf 28 --json
```

批量渲染清单：

```json
{
  "schema": "jianying-render-batch/v1",
  "jobs": [
    {"draft": "./draft-a", "output": "a.mp4"},
    {"draft": "./draft-b", "output": "nested/b.mp4"}
  ]
}
```

```bash
jianying render batch render-jobs.json --out-dir ./previews --json
```

CLI 在启动 FFmpeg 前验证所有草稿、输出唯一性和目录包含关系；输出不能是绝对路径或
逃逸 `--out-dir`。默认 fail-fast 且拒绝覆盖，可显式使用 `--continue-on-error` 和
`--overwrite`。

`jianying-job/v2` 批处理可使用语义明确的别名：

```bash
jianying job batch batch-job.json --out ./batch-output --json
```

它与 `job run` 使用同一个持久化 handler。JSON Lines 服务模式继续使用：

```bash
jianying job serve --state-root ./.jianying-jobs
```

每行请求形如 `{"job":"/path/job.json","out":"/path/output"}`，每行响应都是独立的
成功或结构化错误文档，并携带可恢复 task id。
