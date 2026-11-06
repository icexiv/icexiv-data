#import "../_components/pele/lib.notc" as pele;

# MDL 模型

MDL 保存顶点声明、网格、材质引用、骨骼名称与索引表、部件、shape 替换及 ElementId 挂点。
本文描述 Win32 小端 `0x01000006` 变体。SqPack type-3 条目将这些数据分段存储，
其逻辑文件头重建见 [DAT 模型条目](../sqpack/container-formats.md)。

## 逻辑文件

图中为结构关系。stack 和 runtime 位于 68 字节文件头之后；vertex/index 的文件位置由头部给出。

#pele.diagram(r#"""
flowchart TB
    header[68-byte MDL file header] --> stack[Stack: vertex declarations]
    stack --> runtime[Runtime: strings and metadata tables]
    runtime --> buffers[LOD vertex and index buffers]
    runtime --> names[Materials / attributes / bone names]
    runtime --> shape[Shape index-occurrence replacements]
"""#)

### 文件头

图中位置以 **bit** 为单位，原点为逻辑 MDL 文件起点；多字节整数为小端。

#pele.diagram(r#"""
packet
0-31: "Version"
32-63: "Stack byte length"
64-95: "Runtime byte length"
96-111: "Vertex declaration count"
112-127: "Material count"
128-223: "3 x u32 vertex buffer file offsets"
224-319: "3 x u32 index buffer file offsets"
320-415: "3 x u32 vertex byte lengths"
416-511: "3 x u32 index byte lengths"
512-519: "LOD count"
520-527: "Index streaming flag"
528-535: "Edge geometry flag"
536-543: "Padding"
"""#)

stack 从文件 `0x44` 开始；runtime 从 `0x44 + stack_size` 开始。
长度为解压后的实际字节数。LOD 的 vertex/index offset 是逻辑文件绝对字节偏移。

## 顶点声明与流

stack 中每份声明占 136 字节，容纳 17 个 8 字节 element；stream 为 `0xff` 的 element 终止有效列表。
终止符之后的保留字节不参与顶点取值。mesh 的表内序号关联同序号声明。
下图原点为一个 element 的起点，单位为 bit：

#pele.diagram(r#"""
packet
0-7: "Stream"
8-15: "Byte offset within vertex stride"
16-23: "Scalar type"
24-31: "Usage"
32-39: "Usage index"
40-63: "Reserved"
"""#)

顶点字段的文件寻址为：

```text
vertex_offset[lod] + mesh.stream_offset[stream]
  + vertex_index × mesh.stream_stride[stream] + element.offset
```

| Type ID | 分量布局 | 编码 |
| --- | --- | --- |
| 0 | float1 | 1 × IEEE 754 binary32，4 字节 |
| 1 | float2 | 2 × IEEE 754 binary32，8 字节 |
| 2 | float3 | 3 × IEEE 754 binary32 |
| 3 | float4 | 4 × IEEE 754 binary32 |
| 5 | byte4 | 四个原始 u8，使用语义区分权重与索引 |
| 8 | byte4 normalized | 四个 u8，标量值为 `byte / 255` |
| 13 | half2 | 2 × IEEE 754 binary16 |
| 14 | half4 | 4 × IEEE 754 binary16 |
| 17 | byte8 | 八个原始 u8；八影响蒙皮声明中出现 |

binary32 分量按声明顺序连续存放，每个分量占四个小端字节；float1/float2
元素本身分别占 4/8 字节。元素后的间隙由 mesh stream stride 与其他声明
位置决定，不属于该元素的额外分量。Win32 `2026.10.06.0000.0000` 的图形
输入路径将 type 0/1/2/3 分别对应到 `R32_FLOAT`、`R32G32_FLOAT`、
`R32G32B32_FLOAT`、`R32G32B32A32_FLOAT`。

usage 0 为 position，1 为 blend weights，2 为 blend indices，3 为 normal，4 为 UV，
5 为 tangent/flow 输入，6 为 binormal 输入，7 为 color。
`usage_index` 区分文件声明中的同类多组数据。
例如 `(color,0)` 与 `(color,1)` 是两个独立输入；element 的 stream/offset/type
决定它们各自的物理地址与编码，不能只按 usage 合并或用声明顺序替代 index。
Win32 实例既有同一 mesh 同时存储 color0/color1，也有只有 color1 的声明；
同一 usage 的 index 不保证从零开始连续出现。两组 color 使用 type 8 时，
每组的四个 u8 分别按 normalized byte 解码；它们的 shader 用途由材质决定。
UV 的四个标量可承载两组 UV；布局由声明确定。
UV usage 4 的不同 index 可各有不同 scalar type。Win32 实例包含同一声明的
index 0/1/2 分别使用 half4/half4/half2，也包含 half2 的 index 0 与 normalized
byte4 的 index 5 并存。index 是声明的语义编号，不是数组中某个标量 lane 的
编号；不能只由 index 推定 half2 或固定的连续 UV 对数量。
切线和顶点色的数值编码与 shader 使用含义分别处理；文件并不保证它们是普通乘色参数。
byte4/byte8 权重除以 255 得到权重值，indices 则保持整数、索引 mesh 的 bone palette。
实测四影响与八影响样本的原始权重和均为 255。

### Win32 图形输入语义

Win32 `2026.10.06.0000.0000` 的声明转换路径将文件 usage 对应到以下
图形输入。表中的 n 是文件原始 usage-index；它与 scalar lane 编号无关。

| 文件 usage | 图形输入 |
| --- | --- |
| 0 | POSITION0 |
| 1 | BLENDWEIGHT0 |
| 2 | BLENDINDICES0 |
| 3 | NORMAL0 |
| 4 | TEXCOORDn；该路径的输入名表有 n=0–5 |
| 5 | TANGENT0，tangent/flow 输入 |
| 6 | BINORMAL0 |
| 7 | COLORn；该路径的输入名表有 n=0–1 |

这条路径仅对 usage 4/7 叠加 n，usage 3/5/6 则映射为固定编号。原始文件
可以保留不同 index 的声明；这与图形输入是否存在 NORMAL1/TANGENT1 是
不同问题。不能从 raw usage-index 或 UV 高 index 自动推定额外 flow 输入。

图表示对应关系，不使用字节/bit 坐标：

#pele.diagram(r#"""
flowchart LR
    u5[MDL usage 5] --> s14[Internal input 14] --> tangent[TANGENT0]
    u6[MDL usage 6] --> s15[Internal input 15] --> binormal[BINORMAL0]
"""#)

实际 usage 5 声明包含 normalized byte4。四个标量仍是 `byte/255`；其方向
重映射、长度归一化、分量用途及流向效果属于 shader 的具体消费，不能仅由
TANGENT 名字推出。实际 SHPK 签名包含该输入不保证每个 permutation 都读取它。

## Runtime 与字符串

runtime 以 `u16 string_count`、2 字节保留区、`u32 string_byte_length` 开始，随后为字符串池。
池内字符串以 NUL 终止；各种名称表存储相对池起点的 u32 字节偏移。
字符串不携带 Unicode 编码标志。材质记录可保存 `/mt_…mtrl` 形式的名称或完整游戏路径。

字符串池后接 56 字节 model header，继而为元数据表。下图原点为 model header 起点，单位为 bit：

#pele.diagram(r#"""
packet
0-31: "Radius (f32)"
32-47: "Mesh count"
48-63: "Attribute count"
64-79: "Submesh count"
80-95: "Material count"
96-111: "Bone-name count"
112-127: "Bone-palette count"
128-143: "Shape count"
144-159: "Shape-mesh count"
160-175: "Shape-value count"
176-183: "LOD count"
184-191: "Flags 1"
192-207: "ElementId count"
208-215: "Terrain-shadow mesh count"
216-223: "Flags 2"
224-255: "Model clip-out distance (f32)"
256-287: "Shadow clip-out distance (f32)"
288-303: "Additional bounding-box count (inferred)"
304-319: "Terrain-shadow submesh count"
320-351: "Other byte fields"
352-367: "Bone-palette pool u16 count (including padding)"
368-447: "Other fields / reserved"
"""#)

Flags 2 的数值 bit 4（低位编号从 0 开始）控制三份 extra-LOD 记录的存在。
Flags 的数值位编号与小端字节存储顺序是不同概念。
model header 的 `+0x24` 数值与附加包围盒数量的对应关系由真实样本推断，附加盒的对象关联未知。
`+0x2c` 为该变体的 bone-palette pool u16 总数，包含索引数组的对齐填充。

### 表区次序

| 表或区域 | 单条字节数 | 条数或长度来源 |
| --- | --- | --- |
| ElementId | 32 | ElementId count |
| LOD | 60 | 3 |
| Extra LOD | 40 | Flags 2 bit 4 置位时为 3 |
| Mesh | 36 | Mesh count |
| Attribute name offsets | 4 | Attribute count |
| Terrain-shadow mesh | 20 | Terrain-shadow mesh count |
| Submesh | 16 | Submesh count |
| Terrain-shadow submesh | 12 | Terrain-shadow submesh count |
| Material name offsets | 4 | Material count |
| Bone name offsets | 4 | Bone-name count |
| Bone-palette descriptors | 4 | Bone-palette count |
| Bone-palette pool | 2 | model header `+0x2c` |
| Shape | 16 | Shape count |
| Shape mesh | 12 | Shape-mesh count |
| Shape value | 4 | Shape-value count |
| Submesh bone map | 变长 | u32 字节长度，随后为 u16 索引 |
| 扩展区与对齐 | 变长 | 与模型变体关联 |
| 包围盒 | 32 | 4 + bone count + 附加盒数量 |

人脸样本在 submesh bone map 和包围盒之间包含扩展数据。该区域的业务含义未知。
不能统一按一个 padding 字节跳过此区域，也不能固定假设每份 bone palette 为 132 字节。

## LOD、Mesh 与部件

60 字节 LOD 记录包含以下字段。偏移相对一条 LOD 记录，单位为字节：

| 偏移 | 类型 | 内容 |
| --- | --- | --- |
| `0x00` | u16 × 2 | normal mesh start/count |
| `0x04` | f32 × 2 | model / texture LOD range |
| `0x0c` | u16 × 8 | water、shadow、terrain-shadow、vertical-fog 的 start/count |
| `0x1c` | u32 × 2 | edge geometry size / offset |
| `0x24` | u32 | polygon count |
| `0x28` | u32 | 未知 |
| `0x2c` | u32 × 2 | vertex / index byte size |
| `0x34` | u32 × 2 | vertex / index file offset |

40 字节 extra-LOD 记录的前 16 字节为 light-shaft、glass、material-change、crest-change 的四组
`u16 start/count`；其余 24 字节含义未知。普通 mesh 分类索引同一份 mesh 表；terrain-shadow 有独立记录表。
范围为空时，start 可保留相邻段边界值，无对应记录。

以下 Mesh 图原点为一条 36 字节记录，单位为 bit：

#pele.diagram(r#"""
packet
0-15: "Vertex count"
16-31: "Padding"
32-63: "Index count"
64-79: "Material index"
80-95: "Submesh start"
96-111: "Submesh count"
112-127: "Bone-palette index"
128-159: "Start index (u16-element units)"
160-255: "3 x u32 vertex stream offsets"
256-279: "3 x u8 vertex strides"
280-287: "Stream count in low 2 bits / other flags"
"""#)

Mesh 的 start index 相对 LOD index buffer，以 u16 元素为单位；index 中的值则为 mesh 内局部顶点号。
stream offset 相对 LOD vertex buffer，以字节为单位。stream 字节的低两位给出流数量；高位保留标志。
八影响人脸 mesh 中可观察到 `0x06`，其流数量为 2。静态 mesh 可没有 palette 表，此时 palette-index 字段无绑定目标。

16 字节 Submesh 为 `u32 index_offset`、`u32 index_count`、`u32 attribute_mask`、
`u16 bone_start`、`u16 bone_count`。index_offset 也相对 LOD index buffer。
attribute mask 的低位编号对应本 MDL 的 attribute 名称表；部分场景模型中它承载不同于角色 attribute 名表的原始标记。
同名 attribute 在不同 MDL 的位置可以不同，例如 `atr_ude` 在两份装备模型中
分别为下标 2 和 0。按名称查出的位掩码属于各自文件，不能缓存为全局固定常量。
bone_start/count 指向 submesh bone map。bone_count 为零时，bone_start 可为 `0xffff`。

## Terrain-shadow 数据

Terrain-shadow 使用独立 mesh/submesh 表。每条 mesh 为 20 字节，图中位置以
bit 为单位、原点为记录起点；整数为小端：

#pele.diagram(r#"""
packet
0-31: "Index count: u32"
32-63: "Start index: u32, LOD-relative u16 units"
64-95: "Vertex offset: u32, LOD-relative bytes"
96-111: "Vertex count: u16"
112-127: "Terrain submesh start: u16"
128-143: "Terrain submesh count: u16"
144-151: "Vertex stride: u8"
152-159: "Unknown byte"
"""#)

start index 索引 LOD 的 index buffer，实际 index 值是本 terrain mesh 的局部顶点
编号。vertex offset 同样相对 LOD vertex buffer，但此表没有普通 mesh 的三流
描述和声明表索引。LOD 的 terrain-shadow 范围指向这份独立 mesh 表。

每条 terrain-shadow submesh 为 12 字节。图中原点为 submesh 起点、单位为 bit：

#pele.diagram(r#"""
packet
0-31: "Index offset: u32, LOD-relative u16 units"
32-63: "Index count: u32"
64-79: "Unknown u16"
80-95: "Unknown u16"
"""#)

观察到的 Win32 terrain 顶点 stride 为 8，每条为四个连续小端 binary16。
三个 XYZ 通道与同 LOD 普通 mesh 的 position XYZ 精确对应；第四通道实值为
零，保留原值，不补写为齐次坐标 1。该观察不规定其他 stride 的通道编码。

## Bone palette

一条 descriptor 为 `u16 offset_units` 与 `u16 bone_count`。
索引数组起点为 **本 descriptor 的起点 + offset_units × 4 字节**，包含 bone_count 个 u16 文件级骨名索引。
pool 包含这些数组及其对齐填充。mesh 的 blend indices 先索引 palette，再由 palette 索引 bone-name 表。

#pele.diagram(r#"""
flowchart LR
    vertex[Vertex blend index] --> palette[Mesh-local palette entry]
    palette --> bone[File bone-name index]
    bone --> name[Bone string]
"""#)

## Shape 替换

shape 名称关联每个 LOD 的 shape-mesh 范围。
以下图原点为一条 16 字节 shape 记录，单位为 bit：

#pele.diagram(r#"""
packet
0-31: "Name offset in string pool"
32-79: "3 x u16 shape-mesh starts"
80-127: "3 x u16 shape-mesh counts"
"""#)

12 字节 shape-mesh 为 `u32 mesh_index_offset`、`u32 value_count`、`u32 value_offset`。
前者匹配目标 mesh 的 start index；后两项引用 shape-value 表。
每个 4 字节 shape-value 为 `u16 base_index_occurrence` 与 `u16 replacement_vertex`。

#pele.diagram(r#"""
flowchart LR
    base[Base index occurrence] --> indices[Mesh index buffer]
    indices --> source[Source vertex]
    replace[Replacement vertex number] --> target[Replacement vertex]
    source --> delta[Attribute difference]
    target --> delta
"""#)

base index 指向 mesh 内的 index occurrence；该位置上的 index 值给出原始顶点号。
不同 occurrence 可引用同一原始顶点并指向不同替换顶点，因此需要保持 occurrence 关系。
人脸样本中 occurrence `5828` 所属 mesh 只有 `5580` 个顶点，index 值为 `650`，替换顶点号为 `3100`。

## ElementId 与包围盒

ElementId 记录包含 `u32 id`、`u32 parent_bone_name_offset`、三分量 f32 translation 和三分量 f32 rotation，
共 32 字节。parent 字段指向字符串池中的骨名。rotation 的游戏求值约定需与骨架/绑点语义关联验证。
实样本既含近零浮点残差，也含 rotation 第三分量 `180.0` 与第二分量负零。
这些原值本身不证明角度单位、轴顺序或局部矩阵的乘法次序。

包围盒由两个 float4 构成：min 与 max。表中先为整体、model、water、vertical-fog 四盒，再为骨盒和附加盒。
附加盒的关联对象未知；没有数据的分类可使用零值盒。
扩展区与包围盒的长度关系需结合 model header 计数解释，不能一律将 runtime 末尾的四盒当作整体盒。
half 顶点量化与存储的 f32 包围盒可能有细小边界差；实样本的 X 下界为
`-10.934733390808105`，解码顶点可为 `-10.9375`。严格逐点包含不作为字段解码规则。
