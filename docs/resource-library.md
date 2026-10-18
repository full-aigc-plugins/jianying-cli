# 用户资源库 `jianying resource`

剪映的音乐库、花字库、特效库都在联网接口后面（需要登录态与请求签名），安装目录里
没有可直接读取的目录文件；而剪映 11.6 起保存的草稿本身是加密的。因此本工具**不做
资源下载**，只做一件事：**把用户自己草稿里真实用过的资源沉淀成可检索、可复用的目录**。

## 为什么这样切

| 方案 | 结论 |
|---|---|
| 直连 `lv-pc-api.ulikecam.com` / `effect.snssdk.com` | 需要字节签名与登录态，属于鉴权绕过，不做 |
| 复用客户端本地 agent 接口（`127.0.0.1:50194`、`jianying-agent-cli.exe`） | 需要宿主注入的 `JIANYING_AGENT_RUNTIME_TOKEN`，插件只做适配位，不自取令牌 |
| **从草稿反演**（本模块） | 零网络、零签名、数据来自用户自己的工程，可离线、可测试 ✅ |

## 命令

```bash
# 扫描草稿根，聚合素材使用（只读；加 --write 才写库）
jianying resource mine "<草稿根>" --json
jianying resource mine "<草稿根>" --write --json

# 跨来源合并后的视图
jianying resource list --json
jianying resource list --kind text_shape --query 荧光 --limit 20 --json

# 按名字/id 片段解析成可落盘的标识
jianying resource resolve 荧光 --kind text_shape --json

# 导入外部 bundle（{entries:[...]} 或完整目录）与导出
jianying resource import bundle.json --json
jianying resource export --out resources.export.json --json

# 把库里的一条素材挂到片段上（幂等；text_shape/sticker/video_effect/audio_effect）
jianying resource apply <draft> <segment-id> 荧光 --kind text_shape --json
```

`apply` 只落盘「库里记录过的东西」：写入同 `type`/`effect_id`/`resource_id` 的素材并挂
`extra_material_refs`，不猜测语义。重复挂同一资源返回 `applied: false, reason:
already-applied`，不会产生重复素材。滤镜/转场不走这条命令（结构不同），会明确报错并
指向 `timeline filter` / `timeline transition`。

默认库路径：`$JIANYING_CLI_HOME/resources.json`，否则
Windows 为 `%APPDATA%\jianying-cli\resources.json`，类 Unix 为
`~/.config/jianying-cli/resources.json`；可用 `--catalog` 覆盖。

## 目录格式 `jianying-resource-library/v1`

```json
{
  "schema": "jianying-resource-library/v1",
  "sources": [
    {
      "id": "<草稿根的规范化绝对路径>",
      "scanned_at": 1760000000,
      "root": "<草稿根>",
      "drafts_scanned": 12,
      "entries": [
        {
          "key": "text_shape\u0001111\u0001222",
          "kind": "text_shape",
          "name": "花字-荧光",
          "effect_id": "111",
          "resource_id": "222",
          "md5": "",
          "uses": 5,
          "drafts": 3,
          "last_seen": 1759900000,
          "sample_draft": "<草稿目录>"
        }
      ]
    }
  ]
}
```

- **按来源保存**：同一个根重复扫描是幂等的（按 `id` 覆盖），不会把 `uses` 越滚越大。
- **查询时合并**：`list`/`export`/`resolve` 跨来源求和 `uses`/`drafts`，`last_seen` 取最大。
- `key = kind + effect_id + resource_id`，便于稳定去重。

## 扫描口径

- 递归查找名为 `draft_content.json` 的文件，**只读**。
- 跳过 `Timelines/<uuid>/draft_content.json` 镜像副本；同一 `id` 的草稿只计一次。
- 容忍 UTF-8 BOM。
- 加密草稿（剪映 11.6 常见）不报错、不解密，计入 `drafts_encrypted`：

```json
{"drafts_scanned": 4, "drafts_with_resources": 0, "drafts_encrypted": 4, "drafts_unreadable": 0}
```

## 限制（明确不承诺）

- 不下载、不解析剪映的联网资源目录；没有网络元数据就**没有**名称，只有 id。
- 不解密草稿：11.6 保存后加密的草稿无法反演，需要先用本工具生成/或使用未加密草稿。
- 不推断资源语义：`kind` 直接取素材对象的 `type`，未知类型保持 `unknown`。
