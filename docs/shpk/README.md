#import "../_components/pele/lib.notc" as pele;

# SHPK Shader Package

SHPK 将编译 shader、资源绑定、材质参数默认值、key 默认值和 permutation node 组织在一个文件中。
本文描述 Win32 DX11、版本 `0x0d01` 与 `0x0e01`。元数据中的 ID 关联 MTRL 的 keys、常量与 sampler。

## 分区与文件头

图中位置为 **bit**，原点为逻辑 SHPK 文件起点。多字节整数为小端；两个 offset 为文件绝对字节偏移。

#pele.diagram(r#"""
packet
0-31: "Magic: ShPk"
32-63: "Version = 0x0d01 or 0x0e01"
64-95: "Platform: DX11"
96-127: "File byte length"
128-159: "Shader blob file offset"
160-191: "Name pool file offset"
192-223: "Vertex shader count"
224-255: "Pixel shader count"
256-287: "Material buffer byte length"
288-303: "Material parameter count"
304-319: "Default values present"
320-335: "Constant resource count"
336-351: "Unknown u16"
352-367: "Sampler resource count"
368-383: "Texture resource count"
384-399: "Unordered-access resource count"
400-415: "Unknown u16"
416-447: "System-key count"
448-479: "Scene-key count"
480-511: "Material-key count"
512-543: "Node count"
544-575: "Alias count"
576-607: "Hull shader count"
608-639: "Domain shader count"
640-671: "Geometry shader count"
672-703: "0x0e01 only: Node-alias cluster count"
"""#)

`0x0d01` 头长 84 字节，止于 Geometry count 之后；`0x0e01` 头长 88 字节，
最后四字节为 node-alias cluster count。图中最后一行只属于 `0x0e01`，不能将
`0x0d01` 的首条 shader offset 误当成该字段。
头后为 metadata；shader blob 从第一个 offset 开始，字符串池从第二个 offset 开始，延伸到文件末尾。
metadata 依次包含五类 shader 描述及其资源表、材质参数描述、可选默认值块、全局资源表、key 表、
两个 subview 默认值、node、alias，以及 `0x0e01` 声明的 node-alias cluster。

#pele.diagram(r#"""
flowchart TB
    metadata[Header and metadata] --> code[Shader blob]
    metadata --> names[Name pool]
    parameters[Material parameter IDs and defaults] --> metadata
    keys[System / scene / material / subview keys] --> nodes[Nodes and passes]
    nodes --> code
    mtrl[MTRL parameter and sampler IDs] --> parameters
    mtrl --> resources[Global and per-shader resources]
    resources --> names
"""#)

## Shader 描述

shader 按 vertex、pixel、hull、domain、geometry 的顺序保存。
每条记录以 20 字节前缀开始，下图原点为该前缀，单位为 bit：

#pele.diagram(r#"""
packet
0-31: "Byte offset within shader blob"
32-63: "Stored shader byte length"
64-79: "Constant count"
80-95: "Sampler count"
96-111: "Unordered-access count"
112-127: "Texture count"
128-159: "Unknown u32"
"""#)

前缀后接该 shader 的 constant、sampler、unordered-access、texture 资源记录。
注意其顺序与文件级全局资源表的 constant、sampler、texture、unordered-access 顺序不同。

shader 数据位置相对 shader blob 起点。DX11 vertex 数据在 DXBC 容器前带 8 字节前缀，
该前缀包含在声明长度内；其余实测阶段的 DXBC 从该数据位置直接开始。
前缀的游戏语义未知。DXBC 容器有自己的 magic 和总字节长度，长度与 shader
描述扣除 vertex 前缀后的区间对应。Geometry 实样本的两段数据直接以 DXBC
开始，不带 vertex 的八字节前缀。

## 资源与名称

所有资源描述共用 16 字节布局；图中原点为一条记录，单位为 bit：

#pele.diagram(r#"""
packet
0-31: "Resource ID"
32-63: "Name-pool byte offset"
64-79: "Name byte length excluding NUL"
80-95: "Flags / resource-type value"
96-111: "Binding slot"
112-127: "Size / register count"
"""#)

名称由显式字节长度和 NUL 终止符共同界定。constant buffer 的 size 可用 16 字节 register 表示，
例如实测 `g_MaterialParameter` 的 26 个 register 对应 416 字节 material buffer。
其他资源的 size 语义需结合种类解释。

shader 资源名称 ID 使用反射 IEEE CRC-32，多项式为 `0xedb88320`，初始状态为零，最终不异或。
例如 `g_MaterialParameter → 0x64d12851`、`g_SamplerNormal → 0x0c5ec1f1`。
它与 SqPack 路径使用的 JAMCRC 初始化方式不同。名称本身保留原始字节。

## 材质默认参数与 keys

一条 material parameter 描述为 `u32 ID`、`u16 byte_offset`、`u16 byte_size`，共 8 字节。
它指向 material buffer 中的目标范围；默认值存在标志非零时，描述表后接声明 material buffer 长度的默认值块。
缺少默认值块与块内为零的数值分别表达不同状态。

system、scene、material key 各以 `u32 ID`、`u32 default_value` 的 8 字节记录保存。
subview 的两个默认值只保存 u32 value，其编号是隐式的。
MTRL 可按 ID 覆盖材质默认 keys 和参数；资源绑定仍依赖对应资源表，不按数组位置强行对应不同文件。

## Nodes、passes 与 aliases

两个版本的 node 固定区均为 **32 字节**，后接各 key group 的值及 pass 表。
图中原点为一条 node 起点，单位为 bit：

#pele.diagram(r#"""
packet
0-31: "Selector"
32-63: "Pass count"
64-191: "16 x i8 pass index (-1 = absent)"
192-223: "Unknown u32"
224-255: "Unknown u32"
"""#)

key values 按 system、scene、material、两个 subview 的顺序排列，每值为 u32；
数量取自文件头 key 计数。两项未知 u32 位于 key 表 **之前**，并非 pass 表之后。
这一位置由真实 pass shader-index 边界验证确定。

pass 占 24 字节，包含 `u32 ID`、`u32 vertex_shader_index`、`u32 pixel_shader_index`、三个未知 u32。
shader 索引关联相应阶段的描述表。alias 占 8 字节，保存 `u32 selector` 和 `u32 node_index`。
一份含两个 Geometry shader 的 `0x0e01` 文件中，pass 的三项尾部值形如
`0xffffffff,0xffffffff,0` 或 `0xffffffff,0xffffffff,1`；末项可能关联 Geometry
shader index（推断）。其完整选择规则和前两项含义仍需独立证据。

`0x0e01` 的 node-alias cluster 位于 alias 表与 shader blob 之间；文件头提供其数量。
`0x0d01` 没有这项头字段，五份实际资源在 alias 末尾直接接 shader blob。
该区域的层级和选择规则需要独立证据；保留区域字节不等于解释了其全部映射。
key 默认值、node 的具体值、alias 指向和实际游戏 permutation 求值分别是不同层次的信息。
