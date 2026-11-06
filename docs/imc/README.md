#import "../_components/pele/lib.notc" as pele;

# IMC 模型变体

IMC（Image Change）为模型变体保存材质、部件 attribute、贴花、特效与材质动画选择信息。
一个文件包含默认子集和若干变体子集；每个子集为文件声明的各部件保存一条记录。
本文描述 Win32 小端布局。IMC 无 magic，也不在头部存储格式版本。

## 文件结构

图中位置以 **bit** 为单位，原点为文件起点；所有多字节整数为小端。

#pele.diagram(r#"""
packet
0-15: "Variant count (excluding default)"
16-31: "Part mask"
32-79: "Default subset: first part record"
"""#)

头部占 4 字节。`variant_count` 不含默认子集，`part_mask` 指定部件。
实测武器文件使用 `0x0001`，装备文件使用 `0x001f`。
部件在子集内按启用位由低到高排列；子集依次为默认、变体 1、变体 2，直至声明数量。
`part_count = popcount(part_mask)`，记录区域长度为 `(variant_count + 1) × part_count × 6`。
掩码 bit 编号为 u16 数值的低位 0 到高位 15，与小端字节存储顺序分开理解。
高部件位和非连续掩码的游戏使用情况未知。

#pele.diagram(r#"""
flowchart TB
    header[4-byte header] --> defaults[Default subset: enabled parts in bit order]
    defaults --> first[Variant 1: same part order]
    first --> rest[Variants 2 through count: same part order]
"""#)

给定子集 `s` 和部件位 `p`，其记录在文件中的偏移为：

```text
rank(p) = popcount(part_mask & ((1 << p) - 1))
offset(s, p) = 4 + 6 × (s × part_count + rank(p))
```

部件位需存在于掩码内。默认子集编号为 0；变体编号不是记录区域中的线性记录编号。

## 六字节记录

图中位置以 **bit** 为单位，原点为一条记录的起点：

#pele.diagram(r#"""
packet
0-7: "Material ID"
8-15: "Decal ID"
16-31: "Attribute / sound packed value"
32-39: "VFX ID"
40-47: "Material animation / unknown bits"
"""#)

| 字节偏移 | 类型 | 含义 |
| --- | --- | --- |
| `0x00` | u8 | 材质选择 ID |
| `0x01` | u8 | 贴花选择 ID |
| `0x02` | u16 | 低 10 位为 attribute mask，高 6 位为 sound selector |
| `0x04` | u8 | VFX 选择 ID |
| `0x05` | u8 | 低 4 位为材质动画选择 ID；高位含义未知 |

以下图表示 `+0x02` 的 **u16 数值位域**，按低位到高位编号，并非文件 bit 流顺序：

#pele.diagram(r#"""
---
config:
  packet:
    showBits: true
    bitsPerRow: 16
    bitOrder: ascending
---
packet
0-9: "Attribute mask"
10-15: "Sound selector"
"""#)

attribute 位关联 MDL 的局部 attribute 表，不能跨模型直接比较位编号。
ID 的路径解析依赖模型所属领域与资源目录。
材质动画字节的低半字节与未知高位分开保留。下图原点为记录 `+0x05` 的
u8 数值，单位为 bit，低位编号为 0；单字节字段不涉及多字节端序变换：

#pele.diagram(r#"""
---
config:
  packet:
    showBits: true
    bitsPerRow: 8
    bitOrder: ascending
---
packet
0-3: "Material animation selector"
4-7: "Unknown"
"""#)

材质动画高位的用途未确认；记录保存选择 ID，不包含贴花图像字节或动画轨道。
