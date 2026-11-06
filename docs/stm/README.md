#import "../_components/pele/lib.notc" as pele;

# STM 染色模板

STM 将材质中的染色模板 ID 和 Stain ID 映射为颜色与标量。Win32 的
`0x0101`、`0x0201` 布局使用小端整数，数值采用 IEEE 754 binary16；
颜色的三个分量依次为 R、G、B。

## 文件头和模板索引

位置以 bit 为单位，原点为文件起点：

#pele.diagram(r#"""
packet
0-15: "Magic: MS"
16-31: "Version"
32-47: "Template count N"
48-55: "Color column count"
56-63: "Scalar column count"
"""#)

8 字节头之后依次为 `u32[N]` 模板 ID、`u32[N]` 模板偏移和模板体。
偏移以 **2 字节**为单位，相对于模板体起点 `8 + 8N`。
`0x0101` 为 Legacy，头中的列计数为 0/0，模板使用 3 颜色列与 2 标量列；
`0x0201` 为 Dawntrail，头中计数为 3/9，模板共 12 列。

索引的第 `i` 项由两张表中的同下标元素组成；ID 与偏移各占四字节。
设模板体起点为 `B = 8 + 8N`，模板 `i` 的文件字节位置为
`T_i = B + 2 × offsets[i]`。

| 字段 | 文件字节位置 | 类型 / 长度 |
| --- | --- | --- |
| 模板 ID 表 | `8` | `u32[N]` |
| 模板偏移表 | `8 + 4N` | `u32[N]`，单位为半字 |
| 模板体 | `B = 8 + 8N` | 按偏移寻址 |

## 模板与列

每模板先有 `u16[C]` 累计列末尾位置，再有列载荷。累计位置以 2 字节为单位，
相对于该模板的末尾位置表之后。相邻末尾位置相等表示空列。

每项累计末尾记录的布局如下。位置以 bit 为单位，原点为该记录起点，
记录间距为 16 bit；数值仍按小端读取：

#pele.diagram(r#"""
packet
0-15: "Column end: u16 halfword offset"
"""#)

设列载荷起点为 `P_i = T_i + 2C`，列 `j` 的半字起点为
`j = 0` 时的 `0`，或 `j > 0` 时的 `ends[j − 1]`。其字节范围为
`[P_i + 2 × start, P_i + 2 × ends[j])`。

#pele.diagram(r#"""
flowchart TB
    index[Template ID / body-relative halfword offset] --> ends[C cumulative u16 ends]
    ends --> colors[Diffuse / Specular / Emissive: half3 columns]
    colors --> scalars[Scalar half columns]
"""#)

Legacy 标量依次为 gloss strength、specular strength。Dawntrail 标量索引 1、2
分别为 metalness、roughness，其余列保留原始位置，语义未确认。
单值宽度 `S` 为颜色列 6 字节、标量列 2 字节。列长决定编码：

| 列长 | 编码 | 内容 |
| --- | --- | --- |
| 0 | Zero | 所有值为零 |
| S | Uniform | 一个值重复 |
| 254S | Dense | 254 个连续值，Stain ID 从 1 开始 |
| PS + 254 | Palette | P 个值，随后 254 字节选择区 |

Palette 选择区第 0 字节为 `0xff` 标记；第 1–253 字节对应 Stain ID 1–253。
选择器 0 表示隐式零，1–P 对应调色板的第 1–P 个值。
选择区没有第 254 个 stain 的值。下图表示选择区的前三字节；位置以 bit
为单位，原点为选择区起点，即该列起点之后的 `PS` 字节。后面的选择器
仍各占一字节，Stain 253 位于选择区的字节 253。

#pele.diagram(r#"""
packet
0-7: "Marker: 0xff"
8-15: "Stain 1 selector"
16-23: "Stain 2 selector"
"""#)

MTRL 染色行保存模板 ID、覆盖掩码及通道信息。模板所在 STM 与材质 ColorTable
行数不是一一对应关系；32 行材质也可能引用 Legacy STM 中的模板。
文件末尾可有不被模板引用的填充字节。
