#import "../_components/pele/lib.notc" as pele;

# index、index2 与 DAT

以下描述 Win32、version 1 容器的主索引、index2 碰撞名称表及 DAT 条目。
数值均为小端。图中位置以 **bit** 为单位；校验摘要与其他未说明字段保留原始值。

## 公共头

三类文件均以此头部开始，偏移相对于文件起点：

#pele.diagram(r#"""
packet
0-63: "Magic: SqPack + two NUL bytes"
64-71: "Platform (0 = Win32)"
72-95: "Reserved / padding"
96-127: "Header size"
128-159: "Version (1)"
160-191: "Container type (1 = DAT, 2 = index)"
"""#)

公共头长度位于 `0x0c`，通常为 `0x400`。紧随其后的是索引专用头或 DAT 专用头；定位时使用声明长度。

## index / index2

### 索引头

以下偏移相对于索引专用头起点：

#pele.diagram(r#"""
packet
0-31: "Index header size"
32-63: "Index version"
64-95: "Main table absolute offset"
96-127: "Main table size in bytes"
"""#)

`+0x08` 给出主表的文件绝对偏移，`+0x0c` 给出字节长度。常见公共头和索引头各为 `0x400` 字节，主表从 `0x800` 开始。

### index：16 字节记录

#pele.diagram(r#"""
packet
0-31: "Filename hash"
32-63: "Directory hash"
64-95: "Packed location / flags"
96-127: "Padding"
"""#)

路径转小写后，在最后一个 `/` 处分割；目录不包含末尾斜杠：

```text
exd/item.exh
directory_hash = JAMCRC("exd")      = 0xe39b7999
filename_hash  = JAMCRC("item.exh") = 0xdebb8979
key = (u64(directory_hash) << 32) | filename_hash
```

`JAMCRC(s) = !CRC32_IEEE(s)`。小端记录先存文件名哈希，再存目录哈希。

### index2：8 字节记录

#pele.diagram(r#"""
packet
0-31: "Full path hash"
32-63: "Packed location / flags"
"""#)

键为完整小写路径的 JAMCRC，例如 `JAMCRC("exd/item.exh") = 0xb118d1a9`。两种索引的公共头类型值相同，不能用该值区分记录格式。

### packed：DAT 编号与位置

此图表示 **u32 数值的位域**，从低位到高位排列，不是文件中的存储位序：

#pele.diagram(r#"""
---
config:
  packet:
    showBits: true
    bitsPerRow: 32
    bitOrder: ascending
---
packet
0: "Synonym"
1-3: "DAT ID"
4-31: "Offset in 128-byte units"
"""#)

```text
is_synonym = (packed & 1) != 0
dat_id     = (packed >> 1) & 7
offset     = u64(packed >> 4) * 128
```

普通记录指向同包的 `.dat{dat_id}`，`offset` 为文件绝对偏移。例如 `packed = 0x102` 指向 `dat1` 的 `0x800`。

synonym 记录需要额外的碰撞/别名解析，不能按普通记录解释为 DAT 地址。

### index2 碰撞名称表

Win32 安装 `2026.10.06.0000.0000` 中，index2 专用头的 `+0x54` 为碰撞名称
表的文件绝对字节偏移，`+0x58` 为其字节长度。下图原点为索引专用头的
`+0x50`，位置单位为 bit，小端整数：

#pele.diagram(r#"""
packet
0-31: "Word at index header +0x50"
32-63: "Collision-name table absolute offset"
64-95: "Collision-name table byte length"
"""#)

表内记录固定为 256 字节；以下原点为一条记录起点，单位为 bit：

#pele.diagram(r#"""
packet
0-31: "Full path JAMCRC"
32-63: "Unknown word"
64-95: "Packed DAT location"
96-127: "Sequence word"
128-2047: "NUL-terminated path + trailing bytes"
"""#)

有效记录的完整路径与 hash 字段对应，同 hash 的不同路径具有独立的 DAT
位置。指定安装的 31 个 flagged 主表 hash 各关联两条有效记录；主表 packed
值均为 1。需要用完整路径区分它们，不能将该 1 当作 `dat0@0`。
序号字在这些双记录组中为 0/1；其他用途与跨版本规则不由此推定。

路径区长 240 字节，字符串在首个 NUL 处结束；其余字节不要求清零。观察到
的尾部终止记录含 hash=`0xffffffff`、packed=0、序号=`0xffffffff`、空路径；
第二个未知字可以为 0 或 `0xffffffff`。这种记录不提供资源位置。
辅助表中的未知字与终止记录剩余字节均不能自动改名为零填充。

## DAT

```text
SqPack 公共头
DAT 专用头
...
资源条目 ← 索引给出的绝对偏移
  条目头 + 块表 + 头部填充
  数据块 0：块头 + 载荷 + 对齐空间
  数据块 1：块头 + 载荷 + 对齐空间
  ...
下一个资源条目
...
```

DAT 按资源、按块存储，不是整个文件共用一条压缩流。资源条目类型如下，与公共头的容器类型无关：

| 值 | 类型 |
| --- | --- |
| `1` | 空/占位 |
| `2` | 标准二进制 |
| `3` | 模型专用布局 |
| `4` | 纹理专用布局 |

### 标准条目（type-2）

相对于条目起点，固定前缀为 24 字节：

#pele.diagram(r#"""
packet
0-31: "Entry header size"
32-63: "Entry type (2 = standard)"
64-95: "Total decoded size"
96-159: "Other fields"
160-191: "Block count"
"""#)

`+0x18` 开始为块表，每块一条 8 字节记录：

#pele.diagram(r#"""
packet
0-31: "Block relative offset"
32-47: "Stored span"
48-63: "Decoded size"
"""#)

条目头长度包含块表和头部填充；块偏移相对于条目头末尾：

```text
block_start = entry_start + entry_header_size + block_relative_offset
```

存储跨度可包含块头及对齐空间，不等于压缩载荷长度。

### 数据块

块头的固定前缀为 16 字节：

#pele.diagram(r#"""
packet
0-31: "Block header size"
32-63: "Other field"
64-95: "Compressed size / raw marker 32000"
96-127: "Decoded size"
"""#)

载荷从 `block_start + block_header_size` 开始：

- 压缩长度字段为 `32000`：原始块，按解包长度直接读取。
- 其他受支持值：读取指定压缩长度，以 **raw DEFLATE** 解码，无 zlib/gzip 外壳。

按块表顺序拼接解包结果。每块解包长度应与块表一致，总长度应与条目头一致；读取前检查偏移、跨度和文件边界，解压时检查实际输入消耗与输出长度。

### 纹理条目（type-4）

纹理条目复用 24 字节固定前缀，但块表换成数据组表。单张二维纹理通常每组对应一级 mip；
数组与 cube 可为每个 mip 的各层或各面分别分组。体纹理的多个切片也可合为一组。

#pele.diagram(r#"""
packet
0-31: "Entry header size"
32-63: "Entry type (4 = texture)"
64-95: "Total decoded size"
96-159: "Other fields"
160-191: "Group count"
"""#)

组表从 `+0x18` 开始，每组一条 20 字节记录。以下图的单位为 bit，原点为记录起点；整数为小端：

#pele.diagram(r#"""
packet
0-31: "Body relative offset of the first data block"
32-63: "Total stored span of this group"
64-95: "Decoded size of this group"
96-127: "First index in block span table"
128-159: "Data block count"
"""#)

组表之后是每个数据块对应的一个 `u16` 存储跨度，按组顺序排列，包含块头、载荷与填充。
组记录的 `block_offset` 为首块在此表中的索引；组内跨度之和等于该组存储跨度。
最后一块也有完整存储跨度，不能把它当作压缩载荷长度。

条目体的位置由条目头长度决定：

```text
body = entry_start + entry_header_size
prefix_length = group[0].body_relative_offset
data_start    = body + prefix_length
```

体开头的前 `prefix_length` 字节是输出文件的前缀，纹理条目中即 TEX 文件头。
在已观察的二维、数组、cube、L8 单级 volume 和 8×32×2 BGRA8 六级 volume
条目中，其余内容按组顺序、逐块解包拼接：

```text
output = prefix + decode(block[0]) + decode(block[1]) + ...
```

同一组可包含多个块；较大的图像会拆成多块。上述拼接型样本的输出总长度等于
条目头的解码长度。
实测数组的 448 组对应 64 层 × 7 mip，cube 的 54 组对应 6 面 × 9 mip；
32 切片的 L8 体纹理只有一组、132 块。分组数量不能用作 TEX 的 mip 数量。

另有 BGRA8 六级 volume 条目，其每组声明的解码长度与所有对应块实际解码
长度相等，但前缀加各组解码长度小于条目头的总输出长度。深度 24、32 的
两个条目分别有 1888、2488 字节差额；TEX 头中的 mip 起点按三维逐级尺寸
给出。Win32 `2026.10.06.0000.0000` 的这类条目按组 ordinal 对应 mip，
每组的物理位置仍由组表与块跨度表决定，独立于 TEX 逻辑偏移。
完整性可按每组实际解码长度与该 mip 所有切片所需长度分别判断；完整组的
字节不会因其他组不足而改变，缺失区的像素值则没有由尺寸公式确定。

下图节点中的范围以**字节**为单位，原点为逻辑 TEX 文件起点；组编号以
type-4 组表起点为零。以 `8×32×24` BGRA8 为例：

#pele.diagram(r#"""
flowchart TB
    table[Type-4 group records and block spans] --> g0[Group 0: 24576 decoded bytes]
    table --> g1[Group 1: 1536 decoded bytes]
    table --> g4[Group 4: 8 decoded bytes]
    g0 --> m0[Mip 0: logical bytes 80 through 24655; complete]
    g1 --> m1[Mip 1: logical bytes 24656 through 27727; requires 3072]
    g4 --> m4[Mip 4: logical bytes 28160 through 28167; complete]
"""#)

压缩跨度的尾部填充属于物理容器；它不赋予逻辑 mip 缺口已知内容。TEX
尺寸、原始 mip 数量和 offset 表与各组物理位置各有独立坐标。

数据块通常从 16 字节边界开始；跨度大于块头加压缩载荷时，差额为填充。DAT 文件可能没有公共头：部分安装会把超过约 2 GB 的数据文件切成不带公共头的分段，这些分段从条目数据开始。

### 模型条目（type-3）

模型条目将逻辑 MDL 分为 stack、runtime、三组 LOD 的 vertex、edge geometry 与 index。
条目头保存各段的对齐容量、存储跨度、位置和块范围；逻辑 MDL 的 68 字节文件头由这些元数据与实际解压长度重建。
图中位置为 **bit**，原点为模型条目起点，多字节整数均为小端。

#pele.diagram(r#"""
packet
0-31: "Entry header byte size"
32-63: "Type = 3"
64-95: "Logical MDL byte size (including 68-byte header)"
96-127: "Unknown u32"
128-159: "Unknown u32"
160-191: "MDL version"
192-543: "11 x u32 decoded segment capacities"
544-895: "11 x u32 stored segment spans"
896-1247: "11 x u32 body-relative segment offsets"
1248-1423: "11 x u16 first block indices"
1424-1599: "11 x u16 block counts"
1600-1615: "Vertex declaration count"
1616-1631: "Material count"
1632-1639: "LOD count"
1640-1647: "Index buffer streaming flag"
1648-1655: "Edge geometry flag"
1656-1663: "Padding"
"""#)

| 条目相对字节偏移 | 类型 | 内容 |
| --- | --- | --- |
| `0x18` | u32[11] | 解压后容量，样本中为实际长度向上取整到 128 字节 |
| `0x44` | u32[11] | 存储跨度，包含各压缩块的头和填充 |
| `0x70` | u32[11] | 段起点相对条目体的偏移 |
| `0x9c` | u16[11] | 首块在跨度表中的索引 |
| `0xb2` | u16[11] | 段内块数 |
| `0xc8` | u16 | vertex declaration 数量 |
| `0xca` | u16 | material 数量 |
| `0xcc` | u8 | LOD 数量 |
| `0xcd` | u8 | index buffer streaming 标志 |
| `0xce` | u8 | edge geometry 标志 |
| `0xcf` | u8 | 填充 |
| `0xd0` | u16[] | 每个数据块的存储跨度，包含块头和块间填充 |

五组平行数组都使用以下段编号：

```text
0 stack       1 runtime
2 vertex L0   3 vertex L1   4 vertex L2
5 edge L0     6 edge L1     7 edge L2
8 index L0    9 index L1   10 index L2
```

`body = entry_start + header_size`。每段的块从 `body + segment_offset` 开始，
依次按跨度表推进；块内容沿用 DAT 的 raw/DEFLATE 数据块编码。
实际解压长度取自块头之和，不能把对齐容量直接写入逻辑 MDL，也不能将容量差作为模型数据补零。
空段块数为零；其首块索引可能指向相邻段的索引位置，不表示共享非空块。

逻辑文件按 stack、runtime，再按 LOD 依次组织 vertex、edge、index：

#pele.diagram(r#"""
flowchart TB
    mdl[68-byte logical MDL header] --> stack[Stack]
    stack --> runtime[Runtime]
    runtime --> lod0[LOD 0: vertex / edge / index]
    lod0 --> lod1[LOD 1: vertex / edge / index]
    lod1 --> lod2[LOD 2: vertex / edge / index]
"""#)

非空 edge 段的 Win32 文件语义需独立样本确认；验证样本的 edge 标志与段长度均为零。

#### 重建的 MDL 文件头

图中原点为 **逻辑 MDL 文件** 起点，单位为 bit，数值为小端。
vertex/index offset 是逻辑文件绝对字节偏移，不是 DAT 偏移或跨度表索引。

#pele.diagram(r#"""
packet
0-31: "MDL version"
32-63: "Actual stack bytes"
64-95: "Actual runtime bytes"
96-111: "Vertex declaration count"
112-127: "Material count"
128-223: "3 x u32 vertex file offsets"
224-319: "3 x u32 index file offsets"
320-415: "3 x u32 actual vertex byte lengths"
416-511: "3 x u32 actual index byte lengths"
512-519: "LOD count"
520-527: "Index buffer streaming flag"
528-535: "Edge geometry flag"
536-543: "Padding"
"""#)

stack/runtime 长度以及各 vertex/index 长度均为实际解压长度。
两个实测模型的 stack 分别为 `408` 和 `816` 字节，等于 declaration 数量乘以 `136`；
条目中对应容量为 `512` 和 `896` 字节。两种长度的区别会影响 runtime 与所有 buffer 的寻址。
