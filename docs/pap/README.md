#import "../_components/pele/lib.notc" as pele;

# PAP 动画包

PAP 把名字记录、Havok 动画对象及时间线字节段放在同一资源中。Win32
`0x00020001` 的整数为小端；Havok 段有自己的变长整数编码规则。

## 文件头与名字记录

文件头长 26 字节。图中位置以 bit 为单位，原点为文件起点。三个段偏移的值
均以字节为单位，相对于文件起点。

#pele.diagram(r#"""
packet
0-31: "Magic: pap + space"
32-63: "Version: u32"
64-79: "Record count: u16"
80-95: "Skeleton ID: u16"
96-111: "Unassigned bytes"
112-143: "Name records offset: u32"
144-175: "Havok offset: u32"
176-207: "Timeline offset: u32"
"""#)

每个名字记录为 40 字节，位置以 bit 为单位，原点为记录起点：

#pele.diagram(r#"""
packet
0-255: "Name: 32 bytes, NUL padded"
256-271: "Unassigned u16"
272-287: "Havok container array index: u16"
288-319: "Unassigned bytes"
"""#)

名字按首个 NUL 截止；无 NUL 时占完整 32 字节。记录的 Havok index 是
animation container 的数组位置，不是 remembered 对象 ID。
Havok 段从 Havok offset 延伸到 timeline offset；后者到文件末尾属于时间线段。
该段包含以 end tag 结束的对象流，以及可选的零填充。观察到的 Win32 资源既有
紧接对象流的时间线，也有把时间线起点对齐文件绝对偏移 16 字节的布局；填充
长 0–15 字节，不属于 Havok 对象流。不能将段长度直接当成对象流消费长度。
时间线内部布局及记录 `+32/+36` 字段含义没有独立确认。
零记录的空包可完全没有 Havok 对象流：实样本的 records/Havok offset 均为 26，
timeline offset 为 32，中间仅六字节零填充。零记录不隐含一个默认动画。

下图的顺序从文件低偏移到高偏移；对齐原点为文件起点，单位为字节：

#pele.diagram(r#"""
flowchart LR
    header[PAP header and name records] --> stream[Havok stream through end tag]
    stream --> pad[Optional zero padding: 0 to 15 bytes]
    pad --> timeline[Timeline at stored absolute offset]
"""#)

## Havok 对象关系

对象流使用 [binary tagfile v3](../havok/README.md)。

#pele.diagram(r#"""
flowchart TB
    record[PAP name / Havok index] --> container[hkaAnimationContainer]
    container --> animations[animations array]
    container --> bindings[bindings array]
    bindings --> binding[hkaAnimationBinding]
    binding --> animation[hkaSplineCompressedAnimation]
    animations --> animation
    binding --> map[Transform track to bone indices]
"""#)

binding 的 `animation` 指针应与对应数组项指向同一个动画。binding 另有原始
骨架名字、transformTrackToBoneIndices、floatTrackToFloatSlotIndices 和 blendHint。
非空 transform 映射按轨道顺序给出骨索引；它不改变曲线编码顺序。骨名匹配、
缺省映射策略与 blendHint 的姿态组合行为属于播放端语义，不能由曲线采样替代。
真实 idle 包的 container 动画顺序为对象 `[6,4]`，与流中对象出现顺序不同。

## Spline 压缩动画元数据

`hkaSplineCompressedAnimation` v0 继承动画类型、duration、transform/float track
数量，另存 numFrames、numBlocks、maxFramesPerBlock、maskAndQuantizationSize、
frameDuration、blockDuration、blockInverseDuration、blockOffsets、floatBlockOffsets、
transformOffsets、floatOffsets 及 data 字节数组。字段是否出现由 presence 位决定。

blockOffsets 指向 data 内的块起点。floatBlockOffsets 给出各块内 transform
轨道之后的 float 区起点。相邻块重叠一帧，其起始帧步长为
`maxFramesPerBlock - 1`。真实双块样本的步长为 17，重叠帧独立解码后因量化略有差异。
时间以秒记录，曲线 knot 使用块内帧坐标。末块可以少于最大帧数。

## 块掩码与轨道

块首先按轨道顺序存四字节掩码，再按相同顺序存每轨道的 position、rotation、scale。
以下图以 bit 为单位，原点为单轨道掩码起点：

#pele.diagram(r#"""
packet
0-7: "Quantization byte"
8-15: "Position component flags"
16-23: "Rotation component flags"
24-31: "Scale component flags"
"""#)

quantization 字节的数值位 0–1 选择 position 编码，2–5 选择 rotation 编码，
6–7 选择 scale 编码；位号从最低有效位 0 开始，不表示另一个字节序。
position/scale 的编码 0 为 u8、1 为 u16，使用
`low + (high-low) × code / (2^bits-1)` 还原。

向量 flags 低位 0/1/2 对应静态 XYZ，高位 4/5/6 对应动态 XYZ；
同一分量低位与高位并存时按静态值读取。其余分量为 identity（位移 0、缩放 1）。
位 3/7 的向量含义没有实际样本证据。旋转任意高四位置位时编码整条 quaternion
曲线，否则任意低四位置位时存一个静态 quaternion；全部为零表示单位旋转。

## 曲线布局

动态曲线首先有以下头。图中位置以 bit 为单位，原点为曲线起点：

#pele.diagram(r#"""
packet
0-15: "Last control index: u16"
16-23: "Degree: u8"
24-31: "First knot: u8 (remaining knots follow)"
"""#)

控制点数 `n = last_control_index + 1`，knot 总数为 `n + degree + 1`；knot 非递减，
有效区间为 `[knots[degree], knots[n]]`。有多个分量时共享 knot 和 degree。

向量 knot 后按块内偏移对齐四字节，按 XYZ 顺序读取静态 f32 或动态 low/high
两个 f32；identity 分量不占值字节。随后逐控制点、逐动态轴读取量化整数，
末尾按四字节对齐。静态向量不带曲线头。

旋转 knot 后按 quaternion 编码对齐：polar32/uncompressed 为四字节，
three-component48 为两字节，three-component40 为一字节。随后连续存 n 个
quaternion。静态旋转只有一个编码 quaternion；旋转结束后对齐四字节再读 scale。

| Rotation 编码值 | 字节数 | 内容 |
| --- | --- | --- |
| 0 | 4 | Polar32：10 位径向值、18 位球面格点索引和四个符号位 |
| 1 | 5 | Three-component40：三个 12 位数、2 位缺失分量索引、符号位 |
| 2 | 6 | Three-component48：三个 15 位数，另外三个高位编码索引/符号 |
| 5 | 16 | xyzw 四个 f32 |

40 位编码每分量为 `(code-2047)/(2047×sqrt(2))`；缺失索引在数值 bit 36–37，
符号在 bit 38。48 位编码每分量为 `(code-16383)/(16383×sqrt(2))`；三个小端
u16 的最高位依次提供索引低位、索引高位、符号。缺失分量为带符号的
`sqrt(1-sum(stored²))`，其他三分量按 xyzw 顺序填入。

Polar32 的数值 bit 18–27 为 r，令 `w=1-(r/1023)²`。低 18 位索引 i 给出
`b=floor(sqrt(i))`、极角 `b×π/1022`、方位角 `(i-b²)×π/(4b)`（b=0 时方位角为零）；
XYZ 球面半径为 `sqrt(1-w²)`，bit 28–31 分别翻转 xyzw 符号。

曲线按 B-spline 求值，四元数控制点逐通道插值后归一化。这描述压缩曲线的数值
输出，不意味着执行 quaternion slerp、加法动画叠加、根运动或骨架姿态装配。
浮点轨道区和其他旋转量化编码的布局需各自证据，不能按单位值跳过。
