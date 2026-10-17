# 剪映官方 XMLDraft 协议能力参考（agent 通道）

> 本文档是对剪映专业版 Windows 11.6.0.14552 自带协议文档的**纯抽取**产物，用于校正本仓（来源为 pyJianYingDraft 的能力目录）的字段语义。
> 官方文档根目录（只读）：
> `C:\Users\hnxyh\AppData\Local\JianyingPro\Apps\11.6.0.14552\Resources\canvas_agent\skills\jianying.hub.studio_lyra_xml_timeline_builder_v2\`
> 同时参考 `skills/studio.edit.speaker_cut/SKILL.md`（画布冻结规则）与 `skills/jianying.hub.studio_edit_reference_recall_v2/SKILL.md`（资源召回/asset_type）。
>
> **性质声明**：该文档描述的是剪映**官方 agent 编辑通道（XMLDraft v1 + Selection Expression + DraftEditing）**的协议，不是剪映原生 draft_content.json wire 协议。本文只做抽取与对照，不改本仓任何能力声明；对照结论是"提案"。
> **溯源纪律**：凡官方文档未写明的，本文一律标注"官方文档未定义"，不做推测填充。

---

## 1. 协议版本与根结构

来源：`SKILL.md#版本速查`、`references/timeline.md#element:draft`

- 协议标识 `xmldraft/v1`，状态 `current`；根属性 `<draft version>` 枚举只有 `1`，省略时默认按 `1` 处理。
- 根标签 `<draft>` 的子标签（含最大出现次数约束，`timeline.md` 子标签约束行）：
  - `element:compound`（≤1）、`element:compounds`（≤1）、`element:main`（≤1）、`element:multi`（≤1）
  - `extension_action:lyric_recognize`（≤1）、`extension_action:subtitle_recognize`
- `<main>` 只允许 `segment:image`、`segment:video` 子节点；`<multi>` 允许 lyric_recognize（≤1）、subtitle_recognize、audio、effect、filter、image、material_package、sound、sticker、subtitle、template_package、text、video。

## 2. 片段族与扩展动作枚举全集

来源：`SKILL.md#Segment 片段族`、`references/extensions.md`、`references/no-support.md`

**片段族（segment）全集，共 11 个**：

| 标签 | 所在容器 | 来源 |
| --- | --- | --- |
| `video` | main / multi | `segment-visual.md#segment:video` |
| `image` | main / multi | `segment-visual.md#segment:image` |
| `audio` | 仅 multi | `segment-audio.md#segment:audio`（"必须放入 multi"） |
| `sound` | 仅 multi | `segment-audio.md#segment:sound`（"必须放入 multi"） |
| `text` | 仅 multi | `segment-text-sticker.md#segment:text` |
| `subtitle` | 仅 multi | `segment-text-sticker.md#segment:subtitle` |
| `sticker` | 仅 multi | `segment-text-sticker.md#segment:sticker` |
| `filter` | 仅 multi | `segment-effects.md#segment:filter` |
| `effect` | 仅 multi | `segment-effects.md#segment:effect` |
| `template_package` | 仅 multi | `segment-packages.md#segment:template_package` |
| `material_package` | 仅 multi | `segment-packages.md#segment:material_package` |

另有 `segment:motiongraph_effect`（`no-support.md`，官方 agent 通道明确不支持）。

**扩展动作（extension_action）全集，共 2 个**：`subtitle_recognize`、`lyric_recognize`（`extensions.md`）。

**compound**：`element:compound` / `element:member`（`timeline.md#element:compound`），通过 `members`（成员 ID 列表紧凑表示）与 `member.segment_id`（id_ref）把多个片段组合为一个时间对象；compound 本身挂 `time`、`tracking`。

## 3. `asset_type` 枚举

官方文档**没有给出封闭全集**（"官方文档未定义"完整枚举清单）。文档中实际出现的 `asset_type` 值，按出现位置逐项列出：

| asset_type 值 | 消费方式 | 来源 |
| --- | --- | --- |
| `video_mask` | `<mask ref>`，"不得用名称相似的特效、贴纸或素材包替代" | `SKILL.md#输入与资源来源` |
| `lyric_animation` | `<lyric_recognize animation_ref>`，"不使用普通字幕或文字动画资源替代" | `SKILL.md#输入与资源来源`、`extensions.md#lyric_recognize 编写规则` |
| `subtitle_template` | `<subtitle_recognize subtitle_template_ref>` | `extensions.md#subtitle_recognize`、`reference_recall/references/rules/text.md:102` |
| `text_template`（映射为 subtitle_template） | MaterialRecall 前的别名映射 | `reference_recall` 目录脚本行 `asset_type==='text_template'?'subtitle_template'` |
| `text_glow` | `<text text_glow_ref>`，"新增发光时必填" | `segment-text-sticker.md#text_glow_ref` |
| `anim_text` / `anim_video` / `anim_sticker` | `<animation ref>`；anim_sticker 独立入/出场动画"只给 30%–50% 关键贴纸使用" | `reference_recall/SKILL.md#资源选择`、`reference_recall/references/rules/visual.md:19` |
| `tone` | 音色（TTS/speech） | `reference_recall/SKILL.md#资源选择` |
| `mix_mode` | `<mix_mode ref>`；`asset_type=color_card` 的 query 必须直接写颜色值 | `reference_recall/SKILL.md#资源选择`、`segment-visual.md#property:mix_mode` |
| `color_card` | 纯色背景色卡；"透明度写入 XML `<alpha>`，混合模式单独召回/消费 `mix_mode`" | `reference_recall/SKILL.md#资源选择` |
| `font` | `font_ref` / `translation_font_ref`，"不接受字体名称或未知引用" | `extensions.md#font_ref` |
| `curve_speed` | `<speed_curve ref>`，"预设模式只写 ref" | `timeline.md#property:speed_curve` |
| `matting_stroke` | `<matting stroke_ref>`，"描边须写真实 stroke_ref" | `segment-visual.md#property:matting` |

## 4. 字段语义对照（逐项，标注单位/范围/默认/来源）

### 4.1 draft 根属性（`references/timeline.md#element:draft`）

| 字段 | 类型/单位 | 约束 | 默认 | 来源 |
| --- | --- | --- | --- | --- |
| `width` | number / pixel | 与 height 成对、正整数 | 无 | timeline.md#element:draft 属性表 |
| `height` | number / pixel | 同上 | 无 | 同上 |
| `ratio` | string | 无范围声明 | 无 | 同上 |
| `fps` | number | `minimum_exclusive=0`；"已有草稿未被用户或上游结构化契约明确要求改帧率时沿用当前值" | 无 | 同上 + 编写规则第 2 条 |
| `background_color` | color | — | — | 同上 |
| `loudness_normalize` | enum `on,off` | "不得仅凭存在音轨、BGM、时间线重叠或手工 volume 推断开启" | — | 同上 |
| `version` | enum `1` | 省略按 1 处理 | `1` | 同上 |

### 4.2 time（`references/timeline.md#property:time`）

适用宿主：video/image/audio/sound/text/subtitle/sticker/filter/effect/material_package/template_package/compound。

| 字段 | 单位 | 约束 | 场景 | 来源 |
| --- | --- | --- | --- | --- |
| `start` | ms（裸毫秒，"us、ms、s、min、m 等单位后缀均非法"） | `minimum=0` | absolute_fixed | timeline.md#property:time |
| `duration` | ms | `minimum=0`；"不随 speed 改写"；与 `align_end_to` 互斥 | absolute_fixed | 同上 |
| `material_start` | ms | `minimum=0`；"不是草稿时间线起点" | 全部 | 同上 |
| `speed` | 倍率 | `minimum_exclusive=0`，无上限声明；"text/subtitle/sticker 禁止填写 speed，即使值为 1" | 全部 | 同上 |
| `align_start_to` / `align_end_to` | id_ref | canonical 形如 `segment_id.left/.right`，主轨整体用 `main.left/.right`；锚定区间必须能解析为 `start < end` | aligned_interval | 同上 + 编写规则 |
| `align_start_offset` / `align_end_offset` | ms | 左移为负、右移为正 | aligned_interval | 同上 |

时序硬规则（`timeline.md#property:time 编写规则`）：
- 固定区间 = `start` 或 `align_start_to` + `duration`；锚定区间 = `align_start_to` + `align_end_to`，同一 `<time>` 禁止同时出现 `duration` 与 `align_end_to`。
- multi 内写 `duration` 必须同时写 `start` 或 `align_start_to`；main 片段不写 `start`。
- `material_start > 0` 时 `duration` 上限为 `(素材时长 - material_start) / speed`。
- 音频源区间约束（`segment-audio.md#音频源区间约束`）："源区间结束点 `material_start + duration * speed` 不得超过资源时长"；短音效"只能缩短 duration、换更长素材，或拆成多个各自不越界的合法片段"。

### 4.3 transform（`references/transform.md#property:transform`）

适用宿主：video/image/text/subtitle/sticker。五种场景：`absolute_scale` / `absolute_fit` / `relative_box` / `relative_height` / `text_maximum`。

| 字段 | 单位 | 约束/默认 | 来源 |
| --- | --- | --- | --- |
| `layout` | enum `absolute,relative` | 省略表示相对布局 | transform.md 属性表 |
| `x` / `y` | 归一化（0=画布中心，±1=边界；y "1 为上边界、-1 为下边界"） | 必须配合 layout=absolute；官方未声明数值范围 | 同上 |
| `scale` | 倍率 | `minimum_exclusive=0`，默认 `1`；"0.5 不保证占画布 50%" | 同上 |
| `rotation` | degree | 默认 `0`；"逆时针为负、顺时针为正" | 同上 |
| `width` / `height` | percent（画布占比） | `minimum_exclusive=0`；同时写时 width 优先 | 同上 |
| `fit` | enum `cover,contain,none,scale-down` | 与 scale 互斥 | 同上 |
| `crop` | string（如 `1:1`、`16:9`） | 仅 absolute_scale 场景 | 同上 |
| `crop_position` | enum `left,top,right,bottom,left_top,right_top,left_bottom,right_bottom` | 省略表示居中 | 同上 |
| `crop_rotation` | degree | 默认 `0` | 同上 |
| `justify-self` / `align-self` | enum `start,center,end` | 相对布局锚点 | 同上 |
| `padding-top/bottom/left/right` | percent 字面量 | 必须百分比 | 同上 |
| `offset_x` / `offset_y` | percent | offset_y "正值向上" | 同上 |
| `max_width` / `max_height` | percent | 仅 text/subtitle；与 width/height、同轴双侧 padding 互斥 | 同上 |
| `overflow` | enum `scale-down,wrap`，默认 `scale-down` | 仅 text/subtitle；出现时必须至少给 max_width 或 max_height；wrap 横排需 max_width、竖排需 max_height | 同上 |

关键编写规则：`layout="absolute"` 必须同时给 numeric `x` 和 `y`；相对布局字段是 authoring-only，apply 后按最终几何归一化回写。

### 4.4 video / image 片段属性

| 字段 | 语义 | 来源 |
| --- | --- | --- |
| `volume`（video） | 滑杆值 **0–1000**："0 静音、50 为原始幅度的一半、100 保持原始音量、200 为两倍幅度"，默认 `100`；"30% 写裸数 30，不写 0.3 或 30%" | segment-visual.md#segment:video 属性表 |
| `z_order` | integer，"数值越大越靠前" | 同上 |
| `alpha`（property） | `value` 0–1 裸数，"50% 不透明度写 value=\"0.5\"，不写 50"，必填 | segment-visual.md#property:alpha |
| `canvas.type` | enum `blur,none`；blur 开启模糊背景，none 清除 | segment-visual.md#property:canvas |
| `canvas.blur` | 0–1 裸数；四档参照 0.0625/0.375/0.75/1；"未指定时运行时默认为 0.75" | 同上 |
| `filter.intensity`（property:filter） | **0–100 用户面板百分值**，"70% 强度写 70，不写 0.7 或 70%" | segment-visual.md#property:filter |
| `effect`（property:effect） | 只有 `ref`；"特效不提供通用整体强度属性"，可调参数全部写入 `slider_params`，且"参数名、值域和语义只以本轮 recall 返回的当前 ref 声明为准" | segment-visual.md#property:effect、segment-effects.md#slider_params |
| `slider_params` | 无固定属性，按资源 `metadata.slider_params` 动态声明逐字复制 key，数值必须在声明值域内 | segment-effects.md#slider_params |
| `transition.ref/duration` | duration 单位 ms，`minimum=0`；转场"只能挂在存在后继主轨 video/image 的边界左侧片段内"，最后一个/唯一主轨片段禁止包含 transition；"不参与主轨片段时间线累计" | segment-visual.md#property:transition |
| `mix_mode.mode` | enum `multiply,overlay,screen,darken,lighten,soft_light,hard_light,color_burn,color_dodge,linear_burn,difference,color_filter,off`；**主轨禁止使用**；启用时 ref 必须引用已绑定 mix_mode 素材，mode=off 不需要 ref | segment-visual.md#property:mix_mode |
| `mask.ref` | 必须引用已绑定 `video_mask` 素材，语义须匹配 shape；"不得写 mask_ref、运行时 UUID 或裸 resource_id" | segment-visual.md#property:mask |
| `mask.shape` | enum **10 值**：`rectangle,circle,heart,pentagram,line,mirror,custom,contour,text,heterosexual` | 同上 |
| `mask.width` / `mask.height` | 0–1 裸数，相对**画布**宽高 | 同上 |
| `mask.center_x` / `center_y` | ±1，相对画布中心，0,0 居中；"正上负下"（center_y） | 同上 |
| `mask.feather` | **0–1 裸数**；"禁止套用 matting 的 0 至 100 或 0 至 1000 量纲"；0.05–0.12 常规融合，>0.3 不推荐 | 同上 |
| `mask.rotation` | degree，顺时针为正，默认 `0` | 同上 |
| `mask.invert` | boolean，默认 `false` | 同上 |
| `mask.round_corner` | 0–1 裸数，**只用于 rectangle** | 同上 |
| `mask` 换算公式 | `w=width*canvas_width`；`cx=canvas_width/2+center_x*canvas_width/2`（cy 同理向下取负） | 同上编写规则 |
| `matting.mode` | enum `ai,custom,sam3,none`；模板 media slot 仅 `ai,none` | segment-visual.md#property:matting |
| `matting` 普通素材 feather | **0–100**；expand **-20–20**；模板槽位均 0–1000 | 同上 |
| `matting` 描边 | `stroke_size` 缺省 `35`；size/distance/texture 映射 `effects_adjust_*` | 同上 |
| `face_beauty` | 必含 `category,ref,value,face_mode`；value 0–100；category enum `skin,shape,feature,makeup`；face_mode enum `multi,single`（single 必写非空 face_id） | segment-visual.md#property:face_beauty |
| `body_reshape` | 必含 `ref,value,face_mode`，**禁止写 category**；value 0–100；face_mode 同上 | segment-visual.md#property:body_reshape |
| `animation.animation_type` | enum `in,out,loop,group,caption`，"不得写 combo"；text/subtitle 独立动画只支持 in/out/loop | segment-visual.md#property:animation |
| `animation.duration` / `start` | ms 裸毫秒，`minimum=0` | 同上 |
| `animation.slot_index` | ≥0，仅含 template 的 text/subtitle | 同上 |
| `animation.ease/easing` | "当前会被 Blueprint 忽略，新 XML 不得用它承诺缓动效果" | 同上 |
| `deform` | 仅 video/image/sticker；`type` 固定 `corner_pin`；四角 x,y 数值对必须成组出现 | transform.md#property:deform |
| `tracking` | `tracker_type` enum `motion,surface`（默认 motion）；motion 用 center_x/center_y/width/height/target_seg_id（center ±1、框 0–2）；surface 用四角坐标 + surface_name；"主轨 video/image 不得作为 follower 自跟踪"；运动/平面字段不得混用 | transform.md#property:tracking |
| `speed_curve` | video 专属；preset 场景只写 `ref`（curve_speed），custom 场景 ≥2 个有序 `point`（x 为素材归一化进度 0–1，首点 x=0、末点 x=1，端点容差 1e-9；y 为速度倍率 `minimum_exclusive=0`）；ref 与控制点不得混合 | timeline.md#property:speed_curve |

### 4.5 audio / sound（`references/segment-audio.md`）

| 字段 | 语义 | 来源 |
| --- | --- | --- |
| `volume` | 与 video volume 相同的 0–1000 滑杆口径，默认 `100` | segment-audio.md#segment:audio 属性表 |
| `fade.in` / `fade.out` | **裸毫秒**，`minimum=0`；video 下作用于原声，audio/sound 下作用于该片段；"不把毫秒值当音量百分比" | segment-audio.md#property:fade |
| `beats.level` | integer 0–4，"0 不表示关闭，移除节拍须删除 beats；档位不是每秒节拍数" | segment-audio.md#property:beats |
| `beats.mode` | enum 只有 `1`（"新 XML 启用 beat 时固定写数字字面量 1"） | 同上 |

### 4.6 text / subtitle / sticker（`references/segment-text-sticker.md`）

| 字段 | 语义 | 来源 |
| --- | --- | --- |
| 文案写入位置 | 只能写 `content/sub.text` 或 Selection 改 `content sub` 的 `text`；`content` 不是宿主属性 | segment-text-sticker.md#property:content 编写规则 |
| `sub.size` | "剪映字号数值，不是像素或画布百分比"；预算：竖/方屏标题 12–14、字幕 8–10；横屏标题 14–16、字幕 9–12；"字号按画布方向和宽高比例选择，不随画布像素数同比放大" | segment-text-sticker.md#property:sub 编写规则 |
| `sub.color` | canonical 颜色字段，只接受 `#RRGGBB`；同一 sub 不与 `text_color` 同时输出 | 同上 |
| `sub.text_alpha` | 0–1，"50% 写 0.5，与 text_shadow_alpha 的百分制滑杆不同"，默认 `1` | 同上 |
| `sub.text_bold` / `text_italic` / `text_bold_italic` / `text_underline` | boolean（1/0 写法）；下划线与文字弯曲互斥 | 同上 |
| `sub.text_outline_color` / `text_outline_width` | width 为"面板滑杆而非像素"，`minimum=0` | 同上 |
| `sub.range_start` / `range_end` | 半开区间字符索引，必须 `range_start < range_end` | 同上 |
| `sub.font_ref` | 必须是已绑定字体资源 ID，"不能写未绑定的字体名称" | 同上 |
| 模板槽位 | 宿主含 `<template>` 时 sub 按 `addressing="index"` 连续位置映射；容量以 `metadata.text_template_slot_count` 为准，超出只保留前 N 个并返回 `text_template.slot_overflow` warning | segment-text-sticker.md#property:sub 编写规则 |
| `letter_spacing` / `line_spacing` | **text/subtitle 宿主属性**（禁止写成独立子标签）；"用户说字间距 10 就写 10，不折算成 0.1 或像素" | segment-text-sticker.md#segment:text / #segment:subtitle |
| `text_alignment` | enum `left,center,right,top,bottom`；横排只用 left/center/right，竖排只用 top/center/bottom | 同上 |
| `text_typesetting` | enum `horizontal,vertical` | 同上 |
| `text_glow_ref` | 必须先由 MaterialRecall 以 `asset_type=text_glow` 绑定；strength/range 均为百分制滑杆且 requires text_glow_ref | 同上 |
| `text_shadow*` | color（#RRGGBB）+ **alpha 0–100 百分制滑杆**（"30% 写 30，不套用 text_alpha 的 0.3 写法"）+ angle −180–180 + distance 0–100 + smoothing 0–100（"不使用底层 0 至约 0.1667 的引擎数值"）；首次启用阴影建议同时承接 distance/angle，缺现值参考 8 和 −45 | 同上 |
| `keyword`（subtitle 子标签） | `bold`（true/false）、`color`（#RRGGBB）、`occurrence`（≥1，默认 1）、`text` | segment-text-sticker.md#property:keyword |
| `template.ref` | 文字/字幕模板引用；"不支持用空 ref 移除模板"；已有 template 不可原地替换，换模板须整体替换宿主节点 | segment-text-sticker.md#property:template |
| `speech`（text 子标签） | `ref`（tone 资源）+ `speed`（倍率 `minimum_exclusive=0`，"是生成配音的语速"）+ `slot_index` | segment-text-sticker.md#property:speech |
| `sticker` | 子标签只有 animation/time/tracking/transform | segment-text-sticker.md#segment:sticker |

### 4.7 filter / effect 独立片段（`references/segment-effects.md`）

| 字段 | 语义 | 来源 |
| --- | --- | --- |
| `segment:filter.intensity` | 0–100 面板百分值 | segment-effects.md#segment:filter |
| `segment:effect.apply_type` | enum `main,global` | segment-effects.md#segment:effect |
| 资源类型判定 | "只有 type=effect 才生成 `<effect>`；type=filter 必须改用 `<filter>`。不得按展示名、搜索词或风格文案判断资源类型" | 两个文件的编写规则 |

### 4.8 包类片段（`references/segment-packages.md`）

| 字段 | 语义 | 来源 |
| --- | --- | --- |
| `slot.kind` | enum `media,audio,text,sticker`；media/audio/sticker 用 ref，text 写 content + 文字样式、不写 ref | segment-packages.md#property:slot |
| `slot.source_slot_id` | "模板原始槽位 ID；新 XML 的 canonical 寻址字段" | 同上 |
| `slot.promote_to_slot` | 默认 `false`；"仅在明确要求保留模板内置媒体或音频但暴露为可后续替换槽位时使用" | 同上 |
| `slot.inherit` / `sync_group` | boolean / string | 同上 |
| `template_package` | "必须至少提供一个可由模板 source_slot_id 唯一寻址的 slot；不得猜测模板中不存在的槽位" | segment-packages.md#segment:template_package |
| `material_package` | 只有 id/ref + time | segment-packages.md#segment:material_package |

### 4.9 扩展动作（`references/extensions.md`）

`subtitle_recognize` 关键字段与规则：
- `language` 默认 `Auto`（BCP-47）；`translate_language_code` 省略只写原文，填有效值请求双语；`order` enum `source_top,translation_top` **仅双语可用**。
- `keywords` 默认 `false`，"显式 opt-in"；`keyword_scale` 默认 `1.0`；`keyword_size` 与 keyword_scale 同时提供时 keyword_size 优先并 warning。
- `clear_previous` 默认 `true`；`size` 未明确时控制在 8–12（竖/方 8–10、横屏 9–12），"不要默认写 14"。
- 宿主规则：draft/媒体父标签下**不填 target**；仅旧 XML 的 multi 下必须填 target 且指向 video/audio/sound；"禁止生成 multi 下不带 target 的 subtitle_recognize"。
- 不接受 `recognize_type`、`animation_ref`、`mode`；`text_bold` 按能力版本接受 1/0 或回读后补 `<sub text_bold="1">`。
- `lyric_recognize`：只接受 `id,target,animation_ref`；`animation_ref` 必须绑定 lyric_animation；"歌词识别和歌词动画不支持双语、字体、颜色、字号、阴影、排列或显式 mode 参数"。
- 执行语义（`extensions.md#执行语义`）："extension_action 不是最终时间线片段，而是对整体操作意图的占位声明"；先完成普通修改再执行识别；不能预先引用尚未生成的结果 ID。

### 4.10 Selection 修改协议（`references/selection.md`）——本仓架构对照的背景

- Selector 支持 `tag,#id,>,A B,A B(group),[attr=value],[attr*=…],[attr^=…],[attr$=…],:first-of-type,:last-of-type,:nth-of-type(n),:has()`；`select` 恰好 1 个、`selectAll` ≥1 个。
- Mutation 15 种：`attr,removeAttr,text,appendXml,prependXml,beforeXml,afterXml,replaceWithXml,replaceChildrenXml,appendXmlFile,replaceWithXmlFile,replaceChildrenXmlFile,reorderChildrenById,remove,empty`。
- 资源限制：`expression_bytes=1048576；selector_bytes=8192；selector_groups=64；fragment_bytes=4194304；fragment_nodes=100000；fragment_depth=256；has_predicates_per_selector=8；statements=1000；mutations_per_statement=100`。
- 分段提交规则：影响 ≤3 个去重片段可一次表达式内 `draft.apply();`；>3 或命中数不确定时拆"只保存（无 apply）→ 核对 → 独立 apply"。
- 本仓走 Job/Plan + wire 保真路线，与 Selection/DraftEditing 是**不同编辑通道**；上述限制在本仓无对应物（见对照 JSON）。

## 5. 父子与层级关系（合法嵌套规则汇总）

来源：各 Segment/Property 文件的"父角色"与"子标签"声明。

- `draft → {compound, compounds, main, multi, lyric_recognize(≤1), subtitle_recognize}`。
- `main → {video, image}`；main 片段不写 `start`；main 尾段（最后一个/唯一）禁止 `<transition>`。
- `multi → {全部 11 种 segment + 两个 extension_action}`；audio/sound/text/sticker/filter/effect/template_package/material_package 只能在 multi。
- `video/image → {alpha, animation, body_reshape, canvas, effect(局部), face_beauty, filter(局部), mask, matting, mix_mode, time, tracking, transform, transition}`；video 额外 `fade, speed_curve`，且 video/image 可内嵌 `subtitle_recognize`/`lyric_recognize`（lyric ≤1）；main 轨上的 `filter` 最多 1 个。
- `mix_mode` 主轨禁止；`deform` 只能是 transform 直接子标签且仅 video/image/sticker（text/subtitle 阻断）。
- `text → {animation, content, speech, template, time, tracking, transform}`；`subtitle → {animation, content, keyword, template, time, transform}`（subtitle 无 speech/tracking）。
- `filter/effect 独立片段 → {slider_params, time}`；`slider_params` 也可挂局部 `property:filter` / `property:effect`。
- `template_package → {slot(≥1,source_slot_id 唯一寻址), time}`；`slot → {matting, style}`（matting 模板槽只允许 ai/none）；`material_package → {time}`。
- `matting → brush(≤1)`；`compound → {member, time, tracking}`；`content → sub`（有序同类兄弟节点保持文档顺序）。
- `time` 可挂于 compound，不可挂于 draft/main/multi；`tracking` 可挂 compound。
- 同名标签双角色判定："用'父角色 + 标签'识别角色；同名标签可能同时是 Segment 和 Property"（`SKILL.md#使用前核实`第 6 条）。

## 6. 画布与时序契约

来源：`SKILL.md#编辑范围与素材时间`、`timeline.md#element:draft 编写规则`、`studio.edit.speaker_cut/SKILL.md#发布平台与画布硬门禁`。

1. **width/height 成对强制**："每次编写 Selection 都必须保证候选 target_xml 的 `<draft>` 根节点显式保留或设置成对的正整数 width/height……不得只写 ratio，也不得依赖 current_xml、宿主默认值或 apply 阶段补齐。"
2. **auto 画布**：`global.aspect_ratio` 省略或为 auto 时，空草稿优先采用主轨素材 AnalyzeAsset 返回的 `recommended_canvas` 并显式写入 width/height；"副轨素材不决定画布"。
3. **presentation_aspect_ratio**："是应用像素宽高比和旋转后的可见比例；校验以此真实比例为准，允许偶数像素取整，不把推荐尺寸当成唯一合法分辨率"；素材 width/height 不含非方形像素显示校正。
4. **禁止行为（官方明确写出）**：
   - 禁止只写 ratio；
   - 禁止凭素材直觉写画布："`presentation_aspect_ratio=3:4` 或 `recommended_canvas width=1080 height=1440` 时，必须采用等价 3:4 画布，禁止按素材直觉写 1080x1920"；
   - 收到 `draft_auto_canvas_mismatch` 必须按提示修正并重新检查版式；
   - 已有草稿保留当前画布，除非用户明确要求修改；
   - **fps**："只有在用户或上游结构化契约明确要求改帧率时才修改；未要求时必须沿用当前值，不随画布、素材或导出偏好顺手改写"。
5. **比例变化的联动义务**：根画布比例变化后"必须基于新画布在同一批编辑中检查所有受影响的 video、image、text、subtitle、sticker 节点"并显式调整 transform/canvas/fit/crop；"固定 scale、文字字号、贴纸大小、绝对位置和既有裁剪不得假定会自动重排"。
6. **平台冻结画布**（speaker_cut，红白后处理只接受这三种）：抖音 `9:16 → 1080×1920`；B站 `16:9 → 1920×1080`；小红书 `3:4 → 1080×1440`；"已有草稿仅在实际画布与冻结画幅一致时走 revise；不一致时明确停止：当前通用 DraftEditing 不支持安全迁移已有草稿根画布"。
7. **时间单位总则**："所有时间属性只接受不带单位后缀的裸数字，统一按毫秒解释"（`timeline.md#property:time 编写规则`；brush.at 是例外，见 segment-visual.md）。

## 7. 资源引用契约（blueprint_ref 与白名单）

来源：`SKILL.md#输入与资源来源`、`references/selection.md#Mutation 与参数`。

1. **来源二分**：用户视频/图片/独立音频 → AnalyzeAsset 返回的 `blueprint_ref`；平台音乐/音效/字体/模板/蒙版 → MaterialRecall `resolved_resources[].blueprint_ref`。"不要把用户音频唯一标识当平台 resource_id"。
2. **白名单机制**："写稿前先列出本轮可用 ref 白名单，新增或替换的 `videoN`、`imageN`、`audioN`、`soundN`、`musicN`、`filterN`、`effectN`、`fontN` 等必须逐字命中白名单；禁止按素材数量、自然顺序、资源名称或期望效果猜测不存在的编号。"
3. **禁止构造**："不自行导入、搜索、构造资源引用，也不读取内部资源映射或猜测内部状态"；"示例引用不是可复制的资源"。
4. **未解决资源**："已解决资源可以独立使用，未解决资源只阻断依赖它的编辑"。
5. **selection.md 的 resource_ref 规则**与上述白名单一致（同段禁令，禁止发明 `videoN` 等编号）。
6. MaterialRecall 请求必须带齐 `requirement_id`、`asset_type`、`role`（`reference_recall/SKILL.md#资源选择`）。

## 8. 明确不支持清单（逐条抄自 `references/no-support.md`）

以下是**官方 agent 通道（XMLDraft/DraftEditing）的不支持范围**，命中后"直接说明当前暂不支持；不调用素材分析、资源召回、草稿编辑或其他实现工具，也不做同义改写后重试"。

**Schema 标记 `no-support: true` 的能力：**

1. `element:application_params` —— MotionGraph 特效的应用参数容器。
2. `element:plane` —— MotionGraph 平面目标；可写字段由当前 provider 的参数声明决定。
3. `element:runtime_target` —— MotionGraph 特效的运行时目标容器。
4. `element:text_style` —— MotionGraph 文本样式参数；可写字段由当前 provider 的参数声明决定。
5. `property:keyframe` —— 单个程序化关键帧，时间相对宿主片段起点。
6. `property:keyframes` —— 程序化关键帧集合，按子节点顺序承载 keyframe。
7. `segment:motiongraph_effect` —— MotionGraph 特效片段，包含运行时目标和应用参数。

**额外高频能力：**

8. 人声分离：当前暂不支持分离人声与伴奏。
9. 人声美化：当前暂不支持人声美化、音色润色等处理。

> 注意：第 5、6 条只代表**该 agent 通道**不支持关键帧；剪映产品本身支持关键帧（本仓 wire 层也支持 `timeline.keyframe`）。这是通道差异，不是产品语义冲突。

## 9. 与本仓（pyJianYingDraft 来源）对照的要点

机器可读逐条对照见 `provenance/OFFICIAL_CAPABILITY_MAP.json`。人读摘要：

### 9.1 语义冲突（contradicted，需要显式映射才能互通）

| 项 | 官方（agent 通道） | 本仓（pyJianYingDraft wire） | 证据 |
| --- | --- | --- | --- |
| `filter.intensity` | 0–100 面板百分值（"70 写 70"） | 0..1（`--intensity must be a number in range 0..1`，默认 1.0） | `src/timeline_ops.rs:1515-1518`、`segment-effects.md#segment:filter` |
| `effect` 强度 | **无通用强度属性**，只允许 slider_params 动态参数 | `add_effect` 暴露 `--intensity 0..1` | `src/timeline_ops.rs:1651-1654`、`segment-effects.md#segment:effect 编写规则` |
| `volume`（video/audio/sound） | 0–1000 滑杆（100=原始，200=2 倍） | 线性倍数 0.0–4.0，默认 1.0（=滑杆 100；滑杆/100=倍数，但本仓上限 4.0 只到滑杆 400，官方可到 1000） | `crates/jianying-domain/src/clip_settings.rs:44-45`、segment-visual/audio 音量行 |
| `mask.feather` | **0–1** 裸数，明确禁止 0–100 量纲 | 0.0–100.0 | `crates/jianying-domain/src/mask.rs:115`、`segment-visual.md#mask` |
| `mask.round_corner` | 0–1 | 0.0–100.0 | `mask.rs:121`、同上 |
| `text_shadow_alpha` | **0–100 百分制滑杆**（"30% 写 30"） | `TextShadow.alpha` 校验 0.0–1.0（而 distance/diffuse 是 0–100，与官方 distance/smoothing 对齐） | `crates/jianying-domain/src/text_shadow.rs:65`、`segment-text-sticker.md#text_shadow_alpha` |
| `letter_spacing`/`line_spacing` | 面板滑杆整数值（"行间距 10 就写 10"） | wire 为分数值（示例 `line_spacing: 0.02`） | `src/draft.rs:920`、`src/caption_ops.rs:794`、`segment-text-sticker.md#letter_spacing` |

### 9.2 通道差异（非语义冲突，但需知晓）

- **关键帧**：官方 agent 通道 no-support；本仓 `timeline.keyframe` supported（`provenance/CAPABILITIES.json`）。
- **色度抠图（chroma）**：官方 agent 协议文档**未定义** chroma；本仓 `timeline.chroma` supported 且 intensity 用 0..1（`src/timeline_ops.rs:1144-1148`）。
- **编辑模型**：官方 Selection Expression/DraftEditing/白名单/分段 apply；本仓 Job/Plan + wire 保真。二者资源引用机制不同（blueprint_ref vs resource_id+md5 目录解析）。
- **subtitle_recognize/lyric_recognize**：官方是 XML 扩展动作；本仓 ASR 走 `media.asr_provider/asr_whisper_cpp/asr_ledger` + `captions.*`，能力近似但协议形态不同。

### 9.3 官方未覆盖但本仓已有的能力

字幕双语/关键词样式回读（部分对应）、色度抠图、本地/云 TTS 流水线细节、模板库管理、渲染代理/原生渲染、Job 生命周期、runtime entitlement 等——官方 agent 文档只覆盖"如何用 XML 表达"，不覆盖这些工程面。

### 9.4 官方定义了但本仓尚无对应物的能力（空白点）

`recommended_canvas` / `presentation_aspect_ratio` 事实回传、`loudness_normalize` 草稿级响度开关、`beats`（mode=1/level 0–4）、`speed_curve`（preset ref 或有序控制点）、`tracking`（motion/surface）、`deform`（corner_pin）、`face_beauty`/`body_reshape`（含 face_mode/face_id）、`z_order`、transform 相对布局族（justify-self/align-self/padding-*/offset_*/max_width/max_height/overflow/fit）、`mask` 的 shape 枚举化与画布相对几何（width/height/center 相对画布）、`compound/member`、`text_glow_ref` 精确滑杆口径、`animation` 的 slot_index、模板 `slot` 的 source_slot_id 寻址与 promote_to_slot、字幕模板 subtitle_template_ref、双语 order/translation_* 系列、`text_curve_angle`、matting 的 sam3/custom/brush 与 stroke_ref 描边体系、`apply_type(main/global)`、资源 asset_type + blueprint_ref 白名单机制、时间锚定区间（align_*_to/offset）。

---

## 附：本文档的溯源边界

- 全部字段、枚举、约束、默认值均逐条对应官方文件的小节（文件名 + 小节标题已在各表"来源"列给出）。
- 官方未写明的（如 `x/y` 的硬数值范围、`asset_type` 完闭全集、`transition` 在副轨的语义、chroma、`ratio` 的取值集合），本文均标注"官方文档未定义"，未做填充。
- 机器可读对照：`provenance/OFFICIAL_CAPABILITY_MAP.json`（提案性质，不修改 `CAPABILITIES.json` 既有条目）。
