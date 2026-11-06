#import "../_components/pele/lib.notc" as pele;

# AMB 环境系数格式

FFXIV 的 `.amb` 保存环境方向函数的系数。每组颜色数据包含 R、G、B 三个
通道，每通道九个 float32；相位序列用于一天内的时间变化。九槽对应
`1,y,z,x,xy,yz,3z²−1,xz,x²−y²` 的实球谐形状多项式。
其归一化常量、方向评价及颜色单位见[静态消费证据](../provenance/volume-environment.md)。

以下布局对应版本字段为 1 的两种 variant。Win32 安装样本使用小端；Win64
客户端的序列化路径另有数值换端分支。原始来源、实际样本及调用证据见
[来源页](../provenance/volume-environment.md)。

## 公共文件头

公共头为 8 字节。图中位置单位为 **bit**，原点是文件起点；表中偏移单位
为字节。magic 是四个固定字节，多字节数值有小端与大端写出排列。

#pele.diagram(r#"""
packet
0-31: "Magic: AMB + NUL"
32-47: "Version: u16"
48-55: "Tag: u8"
56-63: "Variant: u8"
"""#)

| 偏移 | 类型 | 含义 |
| --- | --- | --- |
| `0x00` | `u8[4]` | `41 4d 42 00`，即 `AMB\0` |
| `0x04` | `u16` | 版本值 1 |
| `0x06` | `u8` | 写出参数的低8位；常规0/1路径分别小端/大端，对应平台未定 |
| `0x07` | `u8` | 布局 variant，0 或 1 |

端序作用于 u16、u32 与 float32 的存储字节；magic、tag 和 variant 不换端。
版本1的小端字节为 `01 00`，大端为 `00 01`。tag截断写出参数，存在tag=1
却仍写小端数值的参数边界，因此不能单独依靠tag解释任意文件；需结合版本
字节与来源约定，具体证据见[序列化路径](../provenance/volume-environment.md#序列化与文件输出)。
其他 tag、版本或 variant 的含义未知。

## Variant 0：32 组显式相位记录

公共头后依次为九个 float32、32 个 u32 count、各组连续记录。

| 文件偏移 | 长度 | 内容 |
| --- | --- | --- |
| `0x08` | `0x24` | 九个方向权重系数 H |
| `0x2c` | `0x80` | 32 个组的记录数 `count[g]` |
| `0xac` | `0x70 × Σcount[g]` | 按组号顺序连接的记录 |

组 g 的首记录文件偏移为 `0xac + 0x70 × Σ(count[0..g])`，求和不含 g。
每条记录为 `0x70 = 112` 字节；位置图以 **记录起点** 为 bit 0。

#pele.diagram(r#"""
packet
0-287: "R: 9 x float32"
288-575: "G: 9 x float32"
576-863: "B: 9 x float32"
864-895: "Phase seconds: float32"
"""#)

相位位于记录 `+0x6c`，以一天 86400 秒为周期供时间选择与混合使用。
H 单独参与方向权重计算，不包含在每条三通道相位记录中。文件长度的布局
公式为 `0xac + 112 × Σcount[g]`。

## Variant 1：ID 目录与系数记录池

公共头之后是 8 字节目录描述、N 个 8 字节目录项，以及 M 个 `0x6c` 字节
系数记录。系数记录不存储相位字段。

描述图的位置以 **文件起点** 为 bit 0；数值字节序沿用公共头的端序约定。

#pele.diagram(r#"""
packet
0-63: "Common header"
64-79: "Unknown: u16"
80-95: "Directory count N: u16"
96-127: "Coefficient record count M: u32"
"""#)

| 文件偏移 | 类型/长度 | 含义 |
| --- | --- | --- |
| `0x08` | `u16` | 未知字段 |
| `0x0a` | `u16` | 目录项数 N |
| `0x0c` | `u32` | 系数记录数 M；序列化路径以其控制记录写出循环 |
| `0x10` | `8 × N` | 目录 |
| `0x10 + 8 × N` | `108 × M` | 系数记录池 |

目录项图以 **单个目录项起点** 为 bit 0：

#pele.diagram(r#"""
packet
0-15: "ID: u16"
16-31: "Count: u16"
32-63: "First record: u32"
"""#)

`ID` 选择一个系数组，`Count` 给出其记录数，`First record` 是相对于
系数池起点的 **记录索引**。其第 i 条记录位于
`0x10 + 8 × N + 108 × (First record + i)`。目录可以表达重复引用、
重叠或空洞，全部 Count 之和不自动等于 M。

每个系数记录为三个连续的九 float32 块，R/G/B 分别位于 `+0x00`、
`+0x24`、`+0x48`。图以 **系数记录起点** 为 bit 0：

#pele.diagram(r#"""
packet
0-287: "R: 9 x float32"
288-575: "G: 9 x float32"
576-863: "B: 9 x float32"
"""#)

相位由组内序号和 Count 构造；安装样本的 Count 为 1、2 或 24 时，对应
相位为 `i × 86400 / Count` 秒。实际客户端的整数回绕与浮点顺序见
[装载证据](../provenance/volume-environment.md#记录装载与时间消费)。
序列化长度为 `16 + 8 × N + 108 × M`。

## 天空 ID 与纹理的关系

`bgcommon/nature/sky/ambient/skylight.amb` 使用 variant 1。其目录 ID 与
客户端天空表的序号对应，目录 Count 与该表的 u16 参数一致。天空 TEX
的深度是纹理切片数量；二者可为 `(depth,count)=(32,24)`、`(2,1)` 或
`(2,2)`。路径对应与实样本范围见[样本证据](../provenance/volume-environment.md#安装样本)。
