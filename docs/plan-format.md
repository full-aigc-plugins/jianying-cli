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

## 轨道类型与段字段

- **video**：`start_us,duration_us,source,source_start_us?,source_duration_us?,speed(0.1-8),volume(0-4),photo?` + 视觉 `scale,x,y,rotation,opacity` + `keyframes` + `mask{name,size,center_x/y(px),rotation,feather(0-100),invert,rect_width,round_corner}` + `filters[{name,intensity(0-100)}]` + `effects[{name,params{0-100}}]` + `mix_mode` + `animation_in/out/group{name,duration_us?}` + `transition_out{name,duration_us?}` + `fade{in_us,out_us}`（片段原声淡入淡出）+ `chroma{color(#RRGGBBAA),intensity,shadow,edge_smooth,spill(0-100)}`（色度抠图）+ `background_filling{type:blur|color,blur(0-1,四档 0.0625/0.375/0.75/1.0),color(#RRGGBBAA)}`（画布背景填充，仅底层主轨生效）
- **audio**：公共字段 + `fade{in_us,out_us}` + `audio_effects[{name,params}]`
- **text**：`text,size,x,y,color,#RRGGBB,border_color,border_width(0-100, 落盘 ×0.002 对齐 pyJYD),bold,italic,underline,alignment(0/1/2),font` + `background{color,style(1|2),alpha,round_radius,height,width,h/v_offset}` + `shadow{color,alpha,angle(-180..180),distance(0-100),diffuse}` + `styles[{range:[utf16起,末],size,bold,italic,underline,color}]` + `animation_*` + `text_effect{effect_id,resource_id}`（花字直通）+ `bubble{effect_id,resource_id}`（气泡直通；两者落盘均为 text_shape）
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
