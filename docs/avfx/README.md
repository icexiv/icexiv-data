#import "../_components/pele/lib.notc" as pele;

# AVFX 特效资产

AVFX 保存特效的调度、时间线、发射器、粒子、绑定器、效应器、动画曲线、纹理路径
和内嵌几何。Win32（观察到的 Ver `0x20110913`）使用带长度的嵌套块，整份文件是一个 AVFX 根块。
纹理独立存于 [ATEX](../atex/README.md)；内嵌几何使用 AVFX 自己的顶点记录。

## 块布局

图中位置以 bit 为单位，原点为单块起点。长度为小端 u32，不含八字节块头和尾部
对齐字节。名称在文件中逆序存储，短名称以 NUL 补齐四字节。

#pele.diagram(r#"""
packet
0-31: "Reversed block name: 4 bytes"
32-63: "Payload byte length: u32"
64-95: "Payload begins (variable length)"
"""#)

下一个兄弟块位于 `start + 8 + align_up(payload_length,4)`。
例如根名称为文件字节 `XFVA`；逻辑 `Tex` 在样本中的文件字节为 `xeT\0`，反向
显示时带前置 NUL。名字比较应把外侧 NUL padding 与名字内容区分。
块尾对齐字节与负载分别保存。块边界不表示负载一定是子块流：字符串、几何数组、
关键帧数组和 Clip 都有自己的记录布局。

## 根块组织

#pele.diagram(r#"""
flowchart TB
    root[AVFX] --> globals[Ver / global parameters / counts]
    root --> scheduler[Schd: scheduler]
    root --> timeline[TmLn: timelines]
    root --> emitter[Emit: emitters]
    root --> particle[Ptcl: particles]
    root --> binder[Bind: binders]
    root --> effector[Efct: effectors]
    root --> texture[Tex: ATEX path strings]
    root --> model[Modl: embedded geometry]
"""#)

ScCn/TlCn/EmCn/PrCn/EfCn/BdCn/TxCn/MdCn 分别声明上述八种块的数量；
同类块按出现顺序形成索引空间，没有额外的 list 容器。global 参数包括裁剪、
draw layer、reference position/rotation/scale、颜色和 fade 等字段，字段编码
由具体标签确定。不能把所有四字节负载统一解释成浮点数；byte 开关也不能按
四字节读取。未知标签与负载仍是资产数据，应按原块边界保留。

## 调度与时间线

Schd 中有 ItCn、TrCn 和重复的 Item/Trgr。Item/Trgr 负载按 bEna、StTm、TlNo
组成记录；后一个同名块带有前面的累计记录。实际样本中的 Trgr 有 12 个块，
最后一个负载含 12 条记录。不能把每个块中的所有记录分别累加，造成重复调度。

TmLn 保存 LpSt/LpEd、BnNo、TICn/CpCn、Item 和 Clip。时间线 Item 也累计编码，
记录字段为 bEna、StTm、EdTm、BdNo、EfNo、EmNo、Plfm，部分记录另带 ClNo。
索引保留有符号值，样本的 BdNo 有 -1/-2，不能直接转换为无符号数组下标。

Clip 是 164 字节的连续记录，不是子块流。下图以 bit 为单位，原点为 Clip
负载起点；数字为小端，字符串为原始定长字节：

#pele.diagram(r#"""
packet
0-31: "Reversed tag"
32-159: "Four i32 values"
160-287: "Four f32 values"
288-1311: "Four 32-byte strings"
"""#)

这些整数/实数的完整业务含义没有由固定记录长度推定。

## 发射器、粒子、绑定器与效应器

Emit 的 EVT、Ptcl 的 PrVT 和 Efct 的 EfVT 为类型判别字段。它们的 Data
依类型变化；未知类型的 Data 不能套用另一类型的参数或默认行为。
发射器的 ItEm/ItPr 也是累计记录；负载前缀可包含任一类别的先前记录。
粒子的 TC1/TC2/TC3/TC4、TN/TR/TD/TP 描述纹理槽位与参数，UvSt 描述 UV 操作；
TxNo 与 TxN 曲线表示不同形式的纹理选择，不能把后者当一个固定纹理下标。
UvSN 与纹理槽位编号、模型 UV 对数属于不同字段。

Bind 的 PrpS/Prp1/Prp2/PrpG 为绑定属性块，内含 BPT/BPTP、Name、BPID、Pos 等。
BPID 与 owner 的 MDL ElementId/SKLB 关联，不是 AVFX 内的模型数组下标。
Life、CrI、CrIR 在实际资源中既有嵌套块变体，也出现四字节标量；仅凭标签不能
把每个同名块强制递归。资产保存参数和引用，如何执行这些参数由特效运行时决定。

## 曲线

曲线块的标签可以是 X/Y/Z、RGB、SclA、Bri 等；包含 KeyC、BvPr/BvPo、RanT
及 Keys 等字段。Keys 负载由 16 字节记录组成，图中位置以 bit 为单位，原点为
单个 key 起点，全部数字为小端：

#pele.diagram(r#"""
packet
0-15: "Time: i16"
16-31: "Key type: u16"
32-63: "Value X: f32"
64-95: "Value Y: f32"
96-127: "Value Z: f32"
"""#)

RGB 与标量曲线对三个值的解释不同，不能把每条记录一律当成三维位置。
关键帧字段和原始类型值可以独立于播放与曲线插值策略读取。

## 内嵌模型

Modl 的 VDrw/VIdx 是绘制顶点和三角形索引；VEmt/VNum 是发射顶点及对应编号。
两对流分别成对存在。VNum 与 VEmt 数量相同，其数值不等同于 VDrw 的索引范围。

VDrw 每顶点 36 字节。图中位置以 bit 为单位，原点为顶点记录起点；half 为
小端 IEEE binary16，其他通道为原始字节：

#pele.diagram(r#"""
packet
0-63: "Position: four half values"
64-95: "Normal: four biased bytes"
96-127: "Tangent: four biased bytes"
128-159: "Color: four bytes"
160-191: "UV pair 1: two half values"
192-223: "UV pair 2: two half values"
224-255: "UV pair 3: two half values"
256-287: "UV pair 4: two half values"
"""#)

normal/tangent 的整数值为存储 byte 减 128，范围 -128..127；这与直接将 byte
重解释为 i8 不同。UV 区为八个 half，可作为四对读取；部分公开资料把它分成
两组 four-component UV。归一化、轴选择、UV 翻转等运行时处理不能由记录布局代替。
VIdx 为小端 u16，每三个索引构成一个三角形，指向 VDrw 顶点。

VEmt 每记录 28 字节，位置以 bit 为单位，原点为记录起点，实数为小端 binary32：

#pele.diagram(r#"""
packet
0-95: "Position XYZ: three f32"
96-191: "Normal XYZ: three f32"
192-223: "Color: four bytes"
"""#)

VNum 每项为小端 u16，与 VEmt 顺序对应。

## Data 的类型与字段编码

Data 的含义由所属节点及其 EVT/PrVT/EfVT/BnVr 判别字段共同确定。同名标签在
不同节点类型中不一定具有相同编码，尤其不能把全部 `MdNo` 都按 i32 读取。
以下组合见于 Ver `0x20110913` 的实际资源：

| 节点 / 类型值 | Data 结构 | 主要字段 |
| --- | --- | --- |
| Emit / 3 | Sphere model | ROT、GeMT、DivX/DivY、Rads、IjS |
| Emit / 4 | Cylinder model | ROT、GeMT、DivX/DivY、Len、Rad |
| Emit / 5 | Embedded model emitter | MdNo、ROT、GeMT、IjS/IjSR |
| Ptcl / 1 | Powder | bLgt、LgtT、CnOf |
| Ptcl / 2 | Windmill | WUvT |
| Ptcl / 5 | Model | MdNo、MNRv/MNRt/MNRi、FrsT、DLT/PLT、FrC、FrRt、ColB/ColE |
| Ptcl / 12 | Disc | PrtC、PCnU/PCnV、PIFU、Ang、WB/WE、RB/RE、CEI/CEO |
| Ptcl / 13 | Light model | MNO |
| Efct / 0 | Point light | Col、DstS、Rot、Pos、Attn、阴影标志、SCDN/SCDF |
| Bind / 0 | Point binder | SpS |
| Emit / 1、2 | Cone / cone model | ROT，InS/OuS 或 GeMT/DivX/DivY，IjA/IjS 等曲线 |
| Ptcl / 3、4、6 | Line / laser / polyline | Len/Wdt/Wd 曲线、颜色组，polyline 另有点数及连接字段 |
| Ptcl / 8 | Quad | SS、bMP |
| Ptcl / 10、11 | Decal / decal ring | SS，ring 另有 RF 与 WID 曲线 |
| Ptcl / 14 | ModelSkin | FrsT、AuTT、bCM、FrC、FrRt、ColB/ColE、SEM/EEM、UVPD |
| Ptcl / 15 | Dissolve | bRev、BST、DTT、EroR、EdW、EdC、EdCW、ECMP |
| Efct / 1 | Directional light | Amb/Col/Rot 组、Pow 曲线 |
| Efct / 3、4、5、7 | Chromatic / Gaussian / directional / radial blur | 强度、半径等曲线，FSDc/FEDc 浮点距离及 FaBP |
| Efct / 6、9 | Camera quake variants | Att、RdO/RdI 曲线及 Rot/Pos 组 |
| Bind / 1、2、4 | Linear / spline / linear adjust | COF，部分类型带 COFR |
| Bind / 3 | Camera binder | Dst/DstR |

这些表项不是 Data 全部历史变体的枚举。曲线组中未出现的可选字段不等同于
显式的零值；不应从编辑器的 UI 默认值反推文件写入了某个值。
例如 cylinder 的 Len 可以缺省，模型曲线组也可以只包含 ACT/ACTR 而没有轴曲线。

- emitter model 的 MdNo 是四字节小端 i32。
- particle model 的 MdNo 是 byte 列表，每个元素给出一个模型选择值；列表长度
  是该块 payload 的字节数，四字节负载也不能自动当成一个 u32。
- light model 的 MNO 在实际样本中为四字节整数。公开声明也有一字节形式，
  两种宽度必须分别读取，不能让一个错误宽度推进后续块的边界。
- powder 的 bLgt 有一字节形式，CnOf 为四字节 f32；同名 bLgt 在 model 的 Data
  中则为四字节值。
- Point light 的 SCDN/SCDF 是 f32，Attn 和阴影开关则保留原整数值。
- decal 的 SS 是 f32，quad 的 SS 声明为整数；不能仅由同名字段决定类型。
  实际 quad 的 bMP 有一字节 `ff` 值，原始数值与 bool 显示转换应分别保存。

ModelSkin 与 dissolve 的目标标志不是内嵌 Modl 下标；前者以 AuTT、后者以 DTT
保存各自的目标选择值。ModelSkin 的标量曲线与三轴/颜色组具有不同层级：

#pele.diagram(r#"""
flowchart TB
    data[Ptcl 14 Data] --> flags[FrsT / AuTT / bCM]
    data --> curves[FrC / SEM / EEM scalar curves]
    data --> axes[FrRt / UVPD axis groups]
    data --> colors[ColB / ColE color groups]
"""#)

## 发射器累计记录与索引空间

发射器 EmCn/PrCn 分别给出 ItEm 和 ItPr 条目块数量。每条记录以
bEnb 开始，包含 TgtB、CrTm、CrCn、CrPr 及坐标/颜色继承、寿命、延迟等字段。
记录是子块序列，字段增减会改变长度，不能仅按固定 276/288/300/312 字节切分。

#pele.diagram(r#"""
flowchart TB
    emit[Emit ordered item blocks] --> itPr[ItPr: prior records + appended particle record]
    emit --> itEm[ItEm: prior records + appended emitter record]
    itPr --> childParticle[Last record TgtB in Ptcl index space]
    itEm --> childEmit[Last record TgtB in Emit index space]
"""#)

条目块可以先 ItPr、后 ItEm，也可以交错。每块的最后一条记录属于该外层标签；
前缀是此前写入的记录，不保证固定为 emitter 类别。TgtB 的目标索引空间
由条目标签决定，不由数值大小、固定前缀长度或目标节点的文件地址决定。
scheduler/timeline 引用同理指向各自类别的数组：TlNo→TmLn，EmNo→Emit，
EfNo→Efct，BdNo→Bind。负值按原有符号保存；-1/-2 的行为不由范围判断赋义。

## 纹理槽位与范围观察

TC1/TC2/TC3/TC4、TN/TR/TD/TP 各有自己的参数子块。bEna 保留启用值，UvSN
保留该槽位的 UV 选择，TxNo 是静态 i32 纹理选择，TLst 为 byte 列表。
TC1 还可以包含 TxN 曲线，不能以首个关键帧值替代整条曲线。

#pele.diagram(r#"""
flowchart LR
    particle[Ptcl texture slots] --> static[TxNo: signed scalar]
    particle --> masks[TLst: byte list]
    particle --> animated[TxN: curve keys / modes]
    static --> textures[Root Tex index space]
    masks --> textures
    animated --> selection[Time-varying selection policy]
"""#)

范围观察可区分非负且位于表内、负值及非负越界值，但它不等同于特效运行时
有效性证明。一个真实样本的禁用 TP 槽保存 TxNo=26，而根纹理表只有九项；
因此保留该字段并指出范围关系，比无条件拒绝整份资产或删除该槽位更能保持来源。
是否启用、screen-copy 标志和动画选纹策略都需要单独保留。

## Binder 属性槽与宿主选择字段

Bind 可有 PrpS（start）、Prp1、Prp2、PrpG（goal）属性槽，各槽独立保存选择器与
偏移参数。实际资源既有仅 PrpS 的 binder，也有多个属性槽和完全没有属性槽的
camera binder。缺省槽不代表文件显式写入了零值，也不代表另一个槽的字节副本。

| 属性字段 | 编码 | 含义 |
| --- | --- | --- |
| BPT | i32 | 宿主选择值；公开枚举名称 0 Caster、1 Target |
| BPTP | i32 | 目标点选择值；0 Origin、1 FitGround、2 DamageCircle、3 ByName |
| BPID | i32 | Bind point ID；与模型 ElementId 的 ID 属于可关联的标识空间 |
| Name | 变长字节 | 名称、NUL 与内部零填充；允许空负载 |
| GenD / CoUF / BCT | i32 | 生成延迟、坐标更新帧与坐标类型原值 |
| bRng / RnPT | i32 | ring 启用值与进度时间 |
| RnPX / RnPY / RnPZ / RnRd | f32 | ring 三轴参数与半径 |
| Pos | 子块组 | ACT/ACTR 和可选轴曲线等位置参数 |

数值负载为小端四字节值。Name 的 `null\0\0\0\0` 负载长度为八字节，名称字节
为字面量 `null`；缺少 Name、存在空 Name 块和名称为 `null` 是三种不同的编码。
公开写入结构在字符串 NUL 后再有三个零字节并对齐四字节；额外零填充不属于名称。
CoUF 的负值按原值保存，不能一律当成解码失败或没有字段。

#pele.diagram(r#"""
flowchart TB
    property[Bind PrpS / Prp1 / Prp2 / PrpG] --> selector[BPT / BPTP / Name / BPID]
    selector --> point[Owner MDL ElementId: match ID]
    point --> boneName[ElementId parent bone name]
    boneName --> skeleton[Selected SKLB skeleton: match stored bone name]
    point --> local[Stored ElementId translation / rotation]
"""#)

BPID 不是 SKLB 骨数组下标。三个武器样本的 BPID 3/4/5 均匹配各自 MDL 的同 ID
ElementId，而这些记录的父骨名 `n_hara` 匹配所选骨架的第 1 号骨；该骨架只有
两条骨记录。把 BPID 5 直接当 bone index 会误判越界。ElementId 的局部偏移也
随 owner 模型变化，不能由一份全局 ID→固定变换表代替。

这些标识关系不规定 Name 与 BPID 同时存在时的运行时优先级，也不将局部参考
姿态、ElementId 偏移、位置曲线和世界姿态自动合成为一张矩阵。
