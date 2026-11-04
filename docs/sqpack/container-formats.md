#import "../_components/pele/lib.notc" as pele;

# index、index2 与 DAT

以下为 Win32、version 1 容器中用于主索引、标准二进制条目和纹理条目的字段，数值均为小端。图中位置以 **bit** 为单位；未画出的头部字段、辅助表和校验区不在本文范围内。

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

纹理条目复用 24 字节固定前缀，但块表换成等级表，每个等级对应一级 mipmap。

#pele.diagram(r#"""
packet
0-31: "Entry header size"
32-63: "Entry type (4 = texture)"
64-95: "Total decoded size"
96-159: "Other fields"
160-191: "Level count"
"""#)

等级表从 `+0x18` 开始，每级一条 20 字节记录：

#pele.diagram(r#"""
packet
0-31: "Body relative offset of the first data block"
32-63: "Total stored span of this level"
64-95: "Decoded size of this level"
96-127: "Block offset (observed 0, meaning unconfirmed)"
128-159: "Data block count"
"""#)

等级表之后是每个数据块对应的一个 `u16` 跨度，按等级顺序排列。该值是从块起点到下一块起点的距离，包含 16 字节块头和块间对齐；最后一块的跨度含义尚未确认。

条目体的位置由条目头长度决定：

```text
body = entry_start + entry_header_size
prefix_length = level[0].body_relative_offset
data_start    = body + prefix_length
```

体开头的前 `prefix_length` 字节原样复制到输出文件开头，纹理条目中即 TEX 文件头。其余内容按等级顺序、逐块解包拼接：

```text
output = prefix + decode(block[0]) + decode(block[1]) + ...
```

同一等级的数据块可超过一块：单块解包后最多 16000 字节，更大的 mipmap 会拆成多块。输出总长度应等于条目头的解码长度。

已知缺口：`block_offset` 的实际含义、最后一块跨度的语义，以及跨度表在多等级纹理中的完整规则仍需对照更多样本确认。

数据块通常从 16 字节边界开始；跨度大于块头加压缩载荷时，差额为填充。DAT 文件可能没有公共头：部分安装会把超过约 2 GB 的数据文件切成不带公共头的分段，这些分段从条目数据开始。

当前库支持主索引表、type-2 与 type-4 提取；不解析辅助碰撞/名称表、模型专用布局，不验证 SHA-1，匹配到 synonym 或歧义哈希时返回不支持错误。
