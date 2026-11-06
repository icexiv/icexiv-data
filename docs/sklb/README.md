#import "../_components/pele/lib.notc" as pele;

# SKLB 骨架容器

SKLB 是 FFXIV 模型骨架资源的容器，保存辅助区与内嵌 Havok 对象图。
人型、武器、monster 和 demihuman 的骨名、父子索引和局部参考姿态位于 Havok
的 `hkaSkeleton` 中。以下描述 Win32 的 1200、1300 布局，整数为小端。

## 文件组织

#pele.diagram(r#"""
flowchart TB
    head[SKLB header] --> auxiliary[Auxiliary section]
    auxiliary --> havok[Havok binary tagfile]
    havok --> container[hkaAnimationContainer skeleton references]
    havok --> mapper[hkaSkeletonMapper and other objects]
    container --> skeleton[hkaSkeleton: names / parents / referencePose]
    mapper --> other[Additional referenced skeleton objects]
"""#)

文件签名字节为 `62 6c 6b 73`（`blks`），版本字节反向表示四位版本文本：
`0021` 为 1200，`0031` 为 1300。段偏移均为文件绝对字节偏移。

### 1200 文件头

长度 46 字节。位置以 bit 为单位，原点为文件起点：

#pele.diagram(r#"""
packet
0-31: "Magic: blks"
32-63: "Version bytes: 0021"
64-79: "Auxiliary file offset: u16"
80-95: "Havok file offset: u16"
96-367: "Unassigned header bytes"
"""#)

### 1300 文件头

长度 48 字节。位置以 bit 为单位，原点为文件起点：

#pele.diagram(r#"""
packet
0-31: "Magic: blks"
32-63: "Version bytes: 0031"
64-95: "Auxiliary file offset: u32"
96-127: "Havok file offset: u32"
128-383: "Unassigned header bytes"
"""#)

辅助区位于辅助偏移与 Havok 偏移之间。样本中的辅助区以 `hpla` 开头，含计数、
偏移和名字；这些字段的完整关系未确认。其与 Havok 骨索引的对应不能仅凭字符串
相同推断。头内未分配含义的字节也需保留。

## 骨架对象的关联

内嵌流使用 [Havok binary tagfile v3](../havok/README.md)，并以类声明解释数据。
对象 ID 与骨索引属于不同的空间。`hkaAnimationContainer.skeletons` 保存对象引用；
`hkaSkeleton.parentIndices` 保存骨数组内的索引，`-1` 表示根骨。

一个文件可以包含多个 `hkaSkeleton`。例如 c0201 的 animation container 引用对象
3，映射器还引用对象 5、8。不能把“对象图中找到的第一个骨架”当成所有关系的目标，
也不能把 mapper 侧骨架和 animation container 的骨架合并成一张骨表。

`hkaSkeleton` 的主要字段为：

| 字段 | 编码内容 |
| --- | --- |
| name | 字符串引用或字符串字节 |
| parentIndices | packed integer 数组，顺序与 bones 相同 |
| bones | hkaBone 结构数组，字段按列编码 |
| referencePose | 每骨 12 个 f32，Havok QsTransform |
| referenceFloats / floatSlots | 浮点参考值与名字 |
| localFrames | localFrame 对象引用与骨索引组成的结构数组 |

骨名和 lockTranslation 来自 `hkaBone`。lockTranslation 未出现时为默认 false。
referencePose 的骨数与 bones、parentIndices 对应，不能用其他模型的骨名顺序重排
后再解释父索引。

## 局部参考姿态

每个 QsTransform 含三组四通道 f32。图中位置以 bit 为单位，原点为变换记录起点；
各系数为小端 binary32：

#pele.diagram(r#"""
packet
0-31: "Translation X"
32-63: "Translation Y"
64-95: "Translation Z"
96-127: "Translation W / retained lane"
128-159: "Quaternion X"
160-191: "Quaternion Y"
192-223: "Quaternion Z"
224-255: "Quaternion W"
256-287: "Scale X"
288-319: "Scale Y"
320-351: "Scale Z"
352-383: "Scale W / retained lane"
"""#)

四元数通道顺序为 xyzw。referencePose 为局部变换，不包含沿父链组合后的 world
矩阵，也不是用于 skinning 的 inverse bind 矩阵。额外 W 通道是存储数据，应与
三维位置/缩放的语义分别保留；不得默认为文件只存十个浮点数。
