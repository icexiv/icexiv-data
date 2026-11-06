#import "../_components/pele/lib.notc" as pele;

# Excel 表格资源

FFXIV 的 Excel sheet 由一个 EXH 元数据文件和若干 EXD 页面组成。EXH 给出
固定行大小、列类型与偏移、页面 ID 区间和语言；EXD 保存行索引、固定字段及
字符串池。列名和跨表业务关系由外部 schema 给出，不存于 EXH 的列描述中。

## 文件组织与 EXL 清单

逻辑资源路径属于 SqPack 的 `exd/` 类别；以下路径按查找用的小写形式表示，
分页与语言种类取决于 EXH 的声明，不表示每张表都存在所有变体：

```text
exd/root.exl                    表名清单
exd/item.exh                    Item 共同结构
exd/item_0_ja.exd                起始 ID 0 的日语页
exd/item_0_en.exd                起始 ID 0 的英语页
exd/item_{start}_chs.exd         简体中文其他页
exd/{sheet}_{start}.exd         无语言后缀页
```

root.exl 是逗号分隔文本，首行为 `EXLT` 和版本号，其余行为表名与表 ID；
ID `-1` 表示没有可用表 ID。表名用于构造 `exd/{sheet}.exh`，表 ID 与行 ID
不同，不能代替文件名；表名末尾的数字也不是分页编号。
各语言页包含整行数据，不只是文本列。读取关系为：

```
root.exl → 表名 → EXH → 分页、语言 → EXD → 行/子行 → 列值
```

语言声明依赖客户端与表，不能假设每个安装都有英语或全部语言文件。

下文为 Win32 EXH v3 / EXD v2。除语言表项的低位 ID 在前外，头、索引和数值
字段均为大端；字符串保留原始字节，可能包含 SeString 负载。

## EXH 头与元数据表

EXH 固定头长 32 字节。图中位置以 bit 为单位，原点为文件起点：

#pele.diagram(r#"""
packet
0-31: "Magic: EXHF"
32-47: "Version: u16"
48-63: "Fixed row bytes: u16"
64-79: "Column count: u16"
80-95: "Page count: u16"
96-111: "Language entry count: u16"
112-135: "Unassigned fields"
136-143: "Variant: u8"
144-159: "Unassigned field"
160-191: "Declared sheet row count: u32"
192-255: "Unassigned fields"
"""#)

variant 1 为普通行，2 为 parent/subrow 复合键。EXH 的 declared row count 是
整张表的声明数量，不能与只读取一个页面时的实际行数比较；subrow 表计入各
subrow，而非只计 parent ID。

固定头后依次为：

| 区域 | 每项长度 | 内容 |
| --- | --- | --- |
| Columns | 4 字节 | 大端 u16 type ID、大端 u16 固定区字节偏移 |
| Pages | 8 字节 | 大端 u32 start ID、大端 u32 ID 区间长度 |
| Languages | 2 字节 | 低位 ID 字节在前；实际语言 ID 0..7，后一字节为零 |

page 的两个数描述半开区间 `[start, start+length)`，不是该页面实际存在多少行。
例如 CustomTalkDefineClient 的页面区间可覆盖 721890 个 ID，页面却只有 22 个
parent；HousingFurniture 的一个页面区间长度 1795，实际普通行数量为 1793。
ID 可以有空洞。

语言 ID 0/1/2/3/4/5/6/7 对应无语言、日、英、德、法、简体中文、繁体中文、韩文，
文件后缀依次为空、`_ja/_en/_de/_fr/_chs/_cht/_ko`。页面名例如
`exd/item_2500_chs.exd`，其中 2500 是页面起始 ID，不是页面序号。

## 列类型

列偏移均相对于该普通行或 subrow 的固定字段区起点。不同列可以共享同一
字节，packed bool 尤其如此。

| Type ID | 固定区编码 | 字节数 |
| --- | --- | --- |
| 0 | 字符串池相对偏移 u32 | 4 |
| 1 | bool，非零为 true | 1 |
| 2 / 3 | i8 / u8 | 1 |
| 4 / 5 | 大端 i16 / u16 | 2 |
| 6 / 7 | 大端 i32 / u32 | 4 |
| 9 | 大端 IEEE binary32 | 4 |
| 10 / 11 | 大端 i64 / u64 | 8 |
| `0x19..0x20` | packed bool，依次选择数值 bit 0..7 | 1 |

packed bool 的位号从最低有效位 0 开始，是一个 byte 的数值位，不表示把整个
EXD 按另一种字节序读取。64 位字段须保留全部位，不能截成宿主 usize 或 f64。

## EXD 页面与 parent 索引

EXD 头长 32 字节。图中位置以 bit 为单位，原点为 EXD 文件起点；所有数值大端：

#pele.diagram(r#"""
packet
0-31: "Magic: EXDF"
32-47: "Version: u16"
48-63: "Unassigned field"
64-95: "Index byte length: u32"
96-127: "Data-section byte length: u32"
128-255: "Unassigned fields"
"""#)

索引位于 `0x20`，每项八字节，为 parent ID u32 与行记录的**文件绝对字节偏移**
u32。索引项数量为 index length / 8；数据区紧随索引，长度包含其行记录及填充。
样本满足 `file_size = 32 + index_length + data_length`。

索引中的 ID 与行记录的物理顺序是不同概念；索引排序可用于查找，但不能以
ID 差值代替真实偏移。一个 parent 对应一个含长度的记录，记录头长六字节。
图中位置以 bit 为单位，原点为 parent 记录起点：

#pele.diagram(r#"""
packet
0-31: "Payload byte length: u32"
32-47: "Subrow count: u16"
48-79: "Payload begins (variable length)"
"""#)

payload length 不含六字节记录头；字符串与固定字段都必须落在该 parent 的
payload 中，不能越到下一个 parent。

## 普通行与 subrow 结构

variant 1 的 count 为 1，payload 先存 `fixed_row_bytes` 字节固定区，随后为
该行的字符串池。

variant 2 的每条 subrow 是大端 u16 subrow ID 加上 `fixed_row_bytes` 字节固定区。
所有 subrow 固定记录连续排放，其后才是**整个 parent 共享的字符串池**：

#pele.diagram(r#"""
flowchart LR
    header[Parent header: payload length / count] --> first[Subrow ID 0 / fixed fields]
    first --> next[Other stored subrow IDs / fixed fields]
    next --> pool[Shared parent string pool]
    first --> offset[String offset from shared pool]
    offset --> pool
"""#)

设 parent payload 起点为 P，subrow 数量为 N，固定大小为 F，则字符串池起点为：

```
variant 1: P + F
variant 2: P + N * (2 + F)
```

字符串列中的 u32 相对这一共享池，而非相对某条 subrow 的固定区末尾。
例如 CustomTalkDefineClient 的 parent 721028 含四条 subrow，字符串偏移
`0,32,51,82` 均相对同一池；第一条偏移 0 不会指向下一条 subrow 的 ID。
偏移可重复引用同一字符串；偏移后的第一个 NUL 终止该字符串。

subrow ID 是文件存储的键，不要求等于物理序号或连续。空 subrow group 与
缺失 parent 也应区分；单个页面只证明该页面中的存在性，不证明其他页面或
语言资源的读取成功。
