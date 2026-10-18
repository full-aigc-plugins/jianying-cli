# 计划格式 `jianying-cli-plan/v1`

时间一律微秒；相对素材路径按计划文件目录解析；坐标半幅（x/y，字幕惯例 y=-0.78）。

## 顶层

| 字段 | 说明 |
|---|---|
| schema | 恒 `jianying-cli-plan/v1` |
| name | 草稿名（可见单层目录名，<200 字节） |
| canvas | `{width,height(16..8192), fps∈24/25/30/50/60}` |
| tracks | 轨道数组；**主视频轨必须第一轨且首段 0 起连续** |
| allow_vip | 默认 false；true 才允许 VIP 目录成员 |
| maintrack_adsorb | 可选；主轨吸附，落盘 `config.maintrack_adsorb`。省略即模板默认 true（与 pyJianYingDraft 一致） |

## 轨道类型与段字段

- **video**：`start_us,duration_us,source,source_start_us?,source_duration_us?,speed(0.1-8),volume(0-4),photo?,change_pitch?（变速变调 is_tone_modify）,crop?{四角 0-1 坐标 upper/lower_left/right_x/y}` + 视觉 `scale,x,y,rotation,opacity` + `keyframes` + `mask{name,size,center_x/y(px),rotation,feather(0-100),invert,rect_width,round_corner}` + `filters[{name,intensity(0-100)}]` + `effects[{name,params{0-100}}]` + `mix_mode` + `animation_in/out/group{name,duration_us?}` + `transition_out{name,duration_us?}` + `fade{in_us,out_us}`（片段原声淡入淡出）+ `chroma{color(#RRGGBBAA),intensity,shadow,edge_smooth,spill(0-100)}`（色度抠图）+ `background_filling{type:blur|color,blur(0-1,四档 0.0625/0.375/0.75/1.0),color(#RRGGBBAA)}`（画布背景填充，仅底层主轨生效）
- **audio**：公共字段 + `fade{in_us,out_us}` + `audio_effects[{name,params}]`
- **text**：`text,size,x,y,color,#RRGGBB,border_color,border_width(0-100, 落盘 ×0.002 对齐 pyJYD),bold,italic,underline,alignment(0/1/2),font` + `vertical（竖排→typesetting）,letter_spacing,line_spacing（落盘 = 0.02 + 值×0.05）,auto_wrapping（开启后素材类型为 subtitle）,max_line_width(0-1),text_alpha(0-1→global_alpha)` + `background{color,style(1|2),alpha,round_radius,height,width,h/v_offset}` + `shadow{color,alpha,angle(-180..180),distance(0-100),diffuse}` + `styles[{range:[utf16起,末],size,bold,italic,underline,color}]` + `animation_*` + `text_effect{effect_id,resource_id}`（花字直通）+ `bubble{effect_id,resource_id}`（气泡直通；两者落盘均为 text_shape）
- **sticker**：`sticker_id,resource_id`
- **filter/effect 轨（全局作用域）**：`filters[0].name+intensity` 或 `effects[0].name(+params)`

## 目录与授权

能力名大小写敏感，必须命中 `catalogs/*.json`（转场 453/滤镜 1052/字体 798/
画面特效 1097+240/场景音 85/音色 57/声音成曲 6/动画 155+124+123/文本动画
145+97+93/蒙版 6/混合模式 10）。VIP 条目仅当计划 `allow_vip: true`。

## 硬校验

同轨递增不重叠；关键帧线性、首点 at_us=0、≥2 点、speed=1 且 source_start_us=0；
转场仅主视频轨非末段、≤1s；特效参数名必须在目录声明、值 0-100；
文本 styles 区间升序不越界（UTF-16）。

## 简报的镜头切分（`max_shot_us`）

`jianying-brief/v1` 支持 `max_shot_us`：给出后同一段素材会被切成多个连续镜头，
每段源窗口按实际取用长度前移，而不是整段铺满。

```jsonc
{"schema":"jianying-brief/v1","target_duration_us":5000000,
 "max_shot_us":1500000,"media":["long.mp4"]}
```

6s 素材 + 5s 目标 + 1.5s 上限 → 4 个镜头（1.5/1.5/1.5/0.5），
`source_start_us` 依次为 0/1.5s/3s/4.5s。

### `min_shot_us`

与 `max_shot_us` 配对使用：收尾镜头若短于 `min_shot_us`，会被并入**同一素材**的前一个镜头
（总时长不变）。合并可能让该镜头略超 `max_shot_us`，此时报告里会出现
`merged-short-tail` 提示——下限关乎可用性，上限只是节奏偏好。

### `shot_order`

| 值 | 行为 |
|---|---|
| `sequential`（默认） | 按 `media` 顺序，前一个素材吃到饱再换下一个 |
| `round-robin` | 每个镜头换下一个素材，轮流取用 |

三段 3s 素材、目标 6s、单镜头上限 1s：

```
sequential  : a,a,a,b,b,b
round-robin : a,b,c,a,b,c
```

单一素材的长片容易"一眼假"，多素材简报建议用 `round-robin`。
