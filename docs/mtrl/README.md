#import "../_components/pele/lib.notc" as pele;

# MTRL 材质

MTRL 保存 shader package 名称、纹理路径、UV/color-set 名称、shader keys、常量、sampler 绑定，
以及材质颜色表和染色选择表。本文描述 Win32 小端版本 `0x01030000`。
MTRL 的参数 ID 与 SHPK 的默认值、资源名称及 binding 元数据关联。

## 文件头与分区

图中位置为 **bit**，原点为逻辑 MTRL 文件起点；多字节数值为小端。

#pele.diagram(r#"""
packet
0-31: "Version = 0x01030000"
32-47: "File byte length"
48-63: "Dataset byte length"
64-79: "String-pool byte length"
80-95: "Shader-name offset in string pool"
96-103: "Texture count"
104-111: "UV-set count"
112-119: "Color-set count"
120-127: "Additional-data byte length"
"""#)

#pele.diagram(r#"""
flowchart TB
    header[16-byte file header] --> names[Texture / UV / color-set descriptors]
    names --> strings[String pool]
    strings --> additional[Additional data]
    additional --> dataset[Color and dye dataset]
    dataset --> material[12-byte material header]
    material --> keys[Shader keys]
    keys --> constants[Constant descriptors]
    constants --> samplers[Sampler descriptors]
    samplers --> values[Shader-value bytes]
"""#)

file length 和各区长度均为字节数。字符串描述表从 `0x10` 开始：

| 描述项 | 大小 | 字段 |
| --- | --- | --- |
| Texture | 4 | u16 string offset、u16 flags |
| UV set | 4 | u16 name offset、u8 index、u8 unknown |
| Color set | 4 | u16 name offset、u8 index、u8 unknown |

描述项按 texture、UV set、color set 的顺序排列，然后是声明长度的字符串池。
偏移相对字符串池起点，字符串以 NUL 结束。shader name 也使用池内偏移。
Color-set 名称记录和颜色 dataset 是不同区域；有 color-set 名称时，dataset 仍可为空。

Additional data 可长于四字节，实例中有 4 字节的角色参数区和 36 字节的场景
参数区。Win64 `2026.10.06.0000.0000` 的材质消费点将首个小端 u32 的 bit 2
解释为内嵌颜色数据标志，bit 3 为染色数据标志；宽高声明也在这个整数中。
下图为数值位域，低位编号为 0，原点为 additional data 的首个整数，单位为 bit；
这种编号不表示字节存储顺序。

#pele.diagram(r#"""
---
config:
  packet:
    showBits: true
    bitsPerRow: 32
    bitOrder: ascending
---
packet
0-1: "Unknown"
2: "Inline color"
3: "Dye data"
4-7: "Half4 width exponent"
8-11: "Row-count exponent"
12-31: "Unknown / family data"
"""#)

宽度指数 `w=0` 采用 Legacy 缺省的每行 16 个 half、16 行。`w>0` 时，每行
有 `2^w` 组 half4，即 `4×2^w` 个 half，行数为 `2^h`，其中 `h` 是行数指数。
例如 `w=3,h=5` 声明 32 half × 32 行。4 字节之外的附加数据依 shader 家族
变化；未确认字段保留原始数值。

## Shader 参数区域

12 字节 material header 的原点为该区域起点，单位为 bit：

#pele.diagram(r#"""
packet
0-15: "Shader-value byte length"
16-31: "Shader-key count"
32-47: "Constant count"
48-63: "Sampler count"
64-95: "Shader flags"
"""#)

| 记录 | 大小 | 字段 |
| --- | --- | --- |
| Shader key | 8 | u32 ID、u32 value |
| Constant | 8 | u32 ID、u16 value offset、u16 value size |
| Sampler | 12 | u32 ID、u32 flags、u8 texture index、3 字节保留区 |

constant offset/size 相对末尾 shader-value 区，单位为字节；值可解释为小端 f32 标量。
MTRL 中的紧凑值偏移与 SHPK material buffer 中的目标偏移分别保存，由同一 constant ID 建立关联。
sampler ID 关联 SHPK 的资源名称，texture index 关联本文件的路径表。
实测 Legacy 材质中也有 `0xff` texture index；它没有路径表项，不能当作索引 255 去访问纹理列表。
这些 binding 的实际来源需结合 SHPK 和颜色表等生成资源判断。
Sampler flags 与 shader flags 保存采样/材质控制位；没有证据的位不赋予名称。

## ColorTable

两类布局均以 IEEE 754 binary16 保存标量，不做 RGBA8 限幅；大于 1 的值可出现在真实材质中。

典型的固定尺寸负载组合如下；宽高声明和染色标志需与负载长度一起解释，不能
仅由 dataset 总长选择表布局。

| 布局 | 行数 | 每行 half 数 | 颜色区长度 | 可选染色区长度 |
| --- | --- | --- | --- | --- |
| Legacy | 16 | 16 | 512 字节 | 32 字节（16 × u16） |
| Dawntrail | 32 | 32 | 2048 字节 | 128 字节（32 × u32） |

由声明 dataset 长度与数据标志确定负载结构；shader 文件名不决定表布局。
实测 `characterlegacy.shpk` 材质同时存在 16 行 Legacy 表和 32 行 Dawntrail 表。
颜色行排列为 row-major；Dawntrail 的相邻行可作为 A/B 行对使用。

以下图原点为一条 **Legacy 行** 起点，单位为 bit：

#pele.diagram(r#"""
packet
0-47: "Diffuse RGB (3 x half)"
48-63: "Specular strength (half)"
64-111: "Specular RGB (3 x half)"
112-127: "Gloss strength (half)"
128-175: "Emissive RGB (3 x half)"
176-191: "Encoded tile selector (half)"
192-255: "Tile repeat / skew (4 x half)"
"""#)

Dawntrail 每行 64 字节，按 half lane 编号描述如下。每个 lane 占 2 字节：

| Lane | 内容 |
| --- | --- |
| 0–2 / 4–6 / 8–10 | Diffuse / specular / emissive RGB |
| 3 / 7 / 11 / 15 / 17 / 20 / 22–23 | 未知或家族相关值 |
| 12–14 | Sheen rate、tint、aperture |
| 16 / 18 / 19 | Roughness、metalness、anisotropy |
| 21 | Sphere mask |
| 24 | Shader selector，数值以 half 编码 |
| 25 / 26 / 27 | Tile selector、tile alpha、sphere selector |
| 28–31 | Tile matrix 四个标量 |

Tile selector 的 half 值乘以 64 给出带小数的编码选择值；不能把其存储位直接当作 u16 ID。
Shader/sphere selector 也按 half 数值解释。上述字段名称来源于公开格式研究，
真实样本验证覆盖和未证实的 shader 求值含义见独立来源记录。

### Legacy 行与 32-half 中间表

Win64 `2026.10.06.0000.0000` 的内嵌颜色消费路径按源宽度选择转换。
令 `S` 为 16-half 文件行，`C` 为 32-half 中间行；下标单位为 half，分别
以各行起点为零。对应关系如下，复制项保留原始二进制位：

| C lanes | 内容 |
| --- | --- |
| 0–2、8–10 | S 的同下标 lanes |
| 3、4–6、11、26 | 1 |
| 7 | `H(1 − float(S[3]))` |
| 12、13、14 | S[4]、S[5]、S[7] |
| 16 | 0.5 |
| 25 | S[11] |
| 28–31 | S[12–15] |
| 15、17–24、27 | 0 |

`H` 在本表达式的有限非零结果上作 binary16 向零截断；结果范围为
`[-65503,65505]`，非零绝对值至少 `2^-11`，没有 half 非正规结果。
中间减法为 binary32 运算，其舍入状态与 half 写回的截断规则分别作用。
例如源值 20 得到 −19，源值 `2^-24` 得到 `0x3bff`。

32 行表的行号以表起点为零；Legacy 的首 16 行依次转换，后 16 行复制
转换后的行 0。Dawntrail 的 32-half 行保留全部源位。中间表为每行 8 个
RGBA16F texel，共 8×32 texel、2048 字节。结构关系为：

#pele.diagram(r#"""
flowchart TB
    source[Legacy rows 0 through 15] --> converted[Converted rows 0 through 15]
    converted --> tail[Undyed row 0 copied to rows 16 through 31]
    converted --> dye[Per-row stain selection]
    tail --> output[32 rows of 32 half lanes]
    dye --> output
"""#)

## ColorDyeTable

染色表每行给出 STM 模板索引和允许覆盖的属性位；Dawntrail 还含染色通道选择。
以下图表示 **解码后的整数数值位域**，低位编号为 0，不表示文件存储 bit 流顺序。

#pele.diagram(r#"""
---
config:
  packet:
    showBits: true
    bitsPerRow: 16
    bitOrder: ascending
---
packet
0-4: "Legacy override flags"
5-15: "Legacy template ID"
"""#)

Legacy 的低五位依次关联 diffuse、specular、emissive、gloss、specular strength。

#pele.diagram(r#"""
---
config:
  packet:
    showBits: true
    bitsPerRow: 32
    bitOrder: ascending
---
packet
0-11: "Dawntrail override flags"
12-15: "Unknown"
16-26: "Template ID"
27-28: "Dye channel"
29-31: "Unknown"
"""#)

Dawntrail 低 12 位中，bit 0/1/2 为 diffuse/specular/emissive，bit 3 含义未知，
bit 4–11 分别关联 metalness、roughness、sheen rate、sheen tint、sheen aperture、anisotropy、sphere index、sphere mask。
实际覆盖数值来自 STM；MTRL 记录只提供选择与覆盖控制。
染色位义、模板关联与游戏求值属于不同层次的结论；真实样本中的原始位与完整染色行为分别标注验证范围。

### 中间颜色行的覆盖目标

指定 Win64 消费路径使用 32 个 half 的中间行。Legacy 文件的 16-lane 行先经
颜色转换；Dawntrail 文件的 32-lane 行可直接复制到该中间行。因此中间行的
下标不能统一当作原始 Legacy 文件行的下标。

该消费路径另按 shader 身份选择覆盖 profile：`characterlegacy.shpk` 使用
五项 Legacy 表及 `chara/base_material/stainingtemplate.stm`，其他 package
使用十二项 Dawntrail 表及 `chara/base_material/stainingtemplate_gud.stm`。
真实 32 行色表也可引用 `characterlegacy.shpk`。
每个 profile 的目标均以**中间行 half lane** 为单位，原点为行的第一个 half：

| Flags 数值位 | Legacy profile 目标 lanes | Dawntrail profile 目标 lanes |
| --- | --- | --- |
| 0 | 0–2 | 0–2 |
| 1 | 4–6 | 4–6 |
| 2 | 8–10 | 8–10 |
| 3 | 3 | 11 |
| 4 | 7 | 18 |
| 5 | 无表项 | 16 |
| 6 / 7 / 8 | 无表项 | 12 / 13 / 14 |
| 9 / 10 / 11 | 无表项 | 19 / 27 / 21 |

Flags 的三个 RGB 位各关联三个分量，其他表项各关联一个分量。表项表示允许
覆盖的目标，不保证写入发生：消费还检查请求通道、模板是否存在及 Stain 值。
该路径中，flags 位号同时标识所读取的 STM 列编号，前三列各有三个 half，
其余列各有一个 half；选择的 STM 版本与覆盖 profile 对应。
观察到的 specular RGB 路径在三个替换 half 位模式均为 `0x0000` 时保留
原有分量。该判断是原始位比较，不把负零 `0x8000` 或非零值纳入这一条件。
这些规则也不把未知 lane 11 命名成某个已证实的 shader 属性。

Legacy 的 16 条染色记录对应中间行 0–15，按通道 0 解释；后 16 行没有
染色记录。Dawntrail 的每条记录保留两位通道。两个染剂请求分别匹配通道
0、1，通道 2、3 均不匹配。染剂 0 跳过覆盖，模板 ID 在选定 STM 中不存在
也跳过该行。颜色转换及首行扩展发生在染色前，重新求值从文件基础颜色
开始。原生 STM 列助手将 254、255 两个染剂字节作为零值哨兵处理；有效
非零染剂索引的取值与列编码关系见 STM 格式。
