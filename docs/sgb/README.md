#import "../_components/pele/lib.notc" as pele;

# SGB 共享场景与家具依赖

SGB 保存共享场景的 layer group、layer 和实例。家具资源位于
`bgcommon/hou/.../asset/*.sgb`，BG 实例引用绘制 MDL 与碰撞资源，SharedGroup
实例引用另一份 SGB。一个路径可在多个实例中重复出现，每个实例有自己的 ID
和变换；按文件路径去重不能替代场景实例表。

以下为 Win32 `SGB1` / `SCN1` 家具资源的布局。整数和 binary32 系数为小端，
字符串以 NUL 结束，名字和资源路径均为独立偏移引用。它们不需要紧随引用字段。

## 文件与场景头

文件头长 12 字节。图中位置以 bit 为单位，原点为文件起点：

#pele.diagram(r#"""
packet
0-31: "File ID: SGB1"
32-63: "Whole file size: u32"
64-95: "Scene chunk count: u32"
"""#)

家具样本在 `+0x0c` 有一个 SCN1，固定头声明长度为 `0x48`。图中位置以 bit 为
单位，原点为 SCN1 块头起点；它的 payload 原点在块头后八字节（文件 `0x14`）：

#pele.diagram(r#"""
packet
0-31: "Scene ID: SCN1"
32-63: "Fixed scene header size: u32"
64-95: "Layer group offset: i32"
96-127: "Layer group count: u32"
128-575: "Other scene offsets / fields / padding"
"""#)

group offset 相对于 SCN1 的 payload 原点。样本值 `0x40` 指向文件 `0x54`，
另有样本值 `0x200` 指向 `0x214`。`0x48` 不是整个文件后续数据的长度；层、
实例、行为附加数据和字符串可以位于固定头之外。不能据此在 `SCN1 + 0x48`
截断所有引用，也不能按通用 AVFX 块流规则跳到下一个场景。

样本的 scene chunk count 和 layer group count 均为 1；多组的寻址结构须与
单组头区分，不能将某个实现的循环步长直接当成文件规范。

## Layer group、layer 与表项

group 头长 16 字节。图中位置以 bit 为单位，原点为 group 头起点：

#pele.diagram(r#"""
packet
0-31: "Group ID: u32"
32-63: "Name offset: u32, from group"
64-95: "Layer-offset table offset: i32, from group"
96-127: "Layer count: u32"
"""#)

layer-offset 表为 `layer_count` 个 i32。**每个表项相对整个表的起点**，不是相对
该表项本身，也不是相对 group 起点。layer 头长 52 字节，图中位置以 bit 为
单位，原点为 layer 头起点：

#pele.diagram(r#"""
packet
0-31: "Layer ID: u32"
32-63: "Name offset: u32, from layer"
64-95: "Instance-offset table offset: i32, from layer"
96-127: "Instance count: u32"
128-415: "Layer flags / festival / reference-list fields"
"""#)

instance-offset 表同样为 i32 序列，每项相对表起点。实例顺序是表项顺序，不要求
目标地址递增或数据紧邻。layer 名称不一定是模型名；实例数量为零也是一种
实际出现的结构。

#pele.diagram(r#"""
flowchart TB
    scene[SCN1 payload origin] --> group[Group: base-relative pointer]
    group --> ltable[Layer-offset table]
    ltable --> layer[Layer: table-relative pointer]
    layer --> itable[Instance-offset table]
    itable --> instance[Instance: table-relative pointer]
    instance --> string[Name / asset path: instance-relative pointer]
"""#)

layer 后部包含工具可见性标志、festival/phase、temporary/housing/version 字段和
其他引用表的偏移/数量。它们与实例表并列；实例表的结束不意味着所有附加层数据
也在该处结束。

## 实例公共头

公共头长 48 字节。图中位置以 bit 为单位，原点为实例起点；名字偏移是相对
同一实例起点的 u32，变换为小端 f32：

#pele.diagram(r#"""
packet
0-31: "Asset type: u32"
32-63: "Instance ID: u32"
64-95: "Name offset: u32"
96-191: "Translation XYZ: three f32"
192-287: "Rotation: three f32"
288-383: "Scale XYZ: three f32"
"""#)

rotation 存三系数，与 SKLB/PAP 中的 xyzw quaternion 布局不同。这里保留文件
的平移、旋转和缩放通道，不由它们推定旋转合成顺序、父子组合、轴转换或运行时
行为。样本包括负零、非单位缩放及非零平移/旋转。

## BG 与关联 SGB 指针

type 1 为 BG，公共头之后的引用前缀如下。图中位置以 bit 为单位，原点仍是
实例起点；两个偏移均相对实例起点，不是相对该扩展区起点：

#pele.diagram(r#"""
packet
384-415: "Model path offset: u32"
416-447: "Collision path offset: u32"
"""#)

BG 后续还有碰撞配置、显示和其他模型相关数据，不能把 `0x38` 当作整个实例的
长度。空碰撞路径是指针指向的空 NUL 字符串，与指针越界或缺少终止符不同。

type 6 为 SharedGroup，`+0x30` 的 u32 指向关联 SGB 路径，原点也为实例起点。
其余 door、rotation/transform 状态、成员覆盖和移动路径等字段属于类型相关
数据，不与 BG 的扩展区混用。

家具 `fun_b0_m0220.sgb` 的模型指向 general/0143 的 MDL，而 SharedGroup 指向
outdoor/hana 的 SGB；不能根据 SGB 文件名套出同编号的 MDL，也不能只在家具
自己的目录查找关联场景。

## 依赖与未知实例

BG/SharedGroup 的依赖由明确的类型和路径字段决定。字符串池中偶然出现 `.mdl`
或 `.sgb` 后缀，不构成已确认的实例引用。相反，合法的空实例表或只有 marker
的关联场景也无需补出模型。

家具样本还出现 type 3/4/7/68（light、VFX、sound、target marker）。它们的公共
实例头与类型号可以保存，类型相关负载须按各自结构解释。未知类型的存在不能
被报告为“整份场景确认没有其他资源依赖”。关联 SGB 图的循环检测和资源读取
错误需要与文件本身的偏移校验分别处理。
