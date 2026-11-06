# 渲染解析需求覆盖清单

本清单以 xiv-companion 的 `docs/render-book` 和本次确认范围为需求边界。
完成一项要求可用的语义输出、回归测试、格式参考和独立验证证据同时存在。
“缺口”不表示格式不存在；“待验证”不等于验证通过。格式参考与来源记录分别维护。

## 需求与验收

| 资源 | 必需变体与输出语义 | 实现与回归测试 | 验证证据 / 缺口 |
| --- | --- | --- | --- |
| SqPack | Win32 index/index2；type-2/4；type-3 分段重建完整 MDL；缺失与读取错误分离 | `src/sqpack`、`tests/sqpack*.rs`；type-3 精确逻辑头；type-4 全组表/范围与受预算约束的单组读取 | [type-3](provenance/sqpack-model.md)、[十份 MDL](provenance/mdl.md)、[type-2 MTRL/SHPK](provenance/materials.md) 真实结构验证；2/3 LOD、edge 空；[type-4 完整对照](provenance/tex.md)。[273 条短 volume 的完整单 mip](provenance/volume-mips.md)可读取并保留原始头，所有短组请求精确报错；六级完整重建仍有缺口 |
| MDL | 声明驱动的顶点、LOD/mesh 分类、索引、submesh/attribute、骨表、shape index occurrence 替换、ElementId、材质引用 | `src/mdl`、`tests/mdl.rs`；借用表、4/8 影响、scalar 解码、shape delta、独立 terrain-shadow buffer/half4；[格式](mdl/README.md) | [基础十份与六份角色/特效几何样本](provenance/mdl.md)：基础 61597 顶点/34009 shape-value；六份另 17481 普通顶点、36 terrain 顶点/126 terrain indices，非空 shadow/fog/light-shaft 与 ElementId rotation=180 原值已验；[SGB 七份家具模型](provenance/sgb.md)和[十份配饰 MDL](provenance/eqdp.md)亦全字段对照；四份实际 color1/双颜色 MDL 另有13256顶点记录及全语义查询对照。[usage5 的 flow/TANGENT0 身份](provenance/vertex-inputs.md)由客户端路径与实际签名关联。glass、normal1/bitangent1/flow1 实样本、rotation 求值和人脸扩展语义缺口保留 |
| MTRL | shader 名、keys、常量、sampler、纹理引用；Legacy 16 行与 Dawntrail 32 行 ColorTable/ColorDyeTable | `src/mtrl.rs`、`src/mtrl/{color,dye}.rs`；借用描述表、内嵌声明、half/dye 位域，完整 2048 字节转换及双通道染色；[格式](mtrl/README.md) | [真实材质与 SHPK 关联](provenance/materials.md)；[5620 份完整表对照](provenance/legacy-color-conversion.md#完整色表输出与染色关联)含 322 份实际 Legacy、两份实际 32 行 dye、全 65536 half 输入。profile/reset/缺模板有静态依据；受控 544 字节材质验证结构和输出，实际 544 字节资源、动态 MXCSR 与 shader 属性效果保留证据边界 |
| SHPK | 默认 keys、常量、资源名与 sampler 元数据；保留 shader 字节边界 | `src/shpk.rs`、`tests/shpk.rs`；0d01 的 84 字节头/0e01 的 88 字节头，默认参数、资源、五阶段元数据、node/pass/alias；[格式](shpk/README.md) | [十三份 Win32 DX11 包](provenance/materials.md)：8609 shaders，含五份 0d01 和一份含两个 Geometry shader 的 0e01；全部元数据、字节边界与 DXBC 长度独立比较，64 份目录头库存已固定。非空 hull/domain 缺少样本；alias cluster 保留原始区，不执行 shader |
| TEX | mip、2D、array、cube、volume；整数/BC 与非截断浮点数据 | `src/tex`、`tests/tex{,_layouts,_mip}.rs`；独立原始头及完整所选 mip 绑定、RGBA8/RGBA32F、原始 mip 身份；通用 MIT BC6H_SF16、块边裁切与预算；[格式](tex/README.md) | [基础十份及两份 volume](provenance/tex.md)：1291 图像、全部像素与 8192 浮点标量位。[单 mip 对照](provenance/volume-mips.md)：678 个完整 mip、7917968 RGBA 字节独立一致，966 个不完整 mip 拒绝，含 273 个短资源 base。[BC6H](provenance/bc6h.md) 的实际零 cube、官方 mode11 及全模式 CPU oracle 有独立数值对照。273 volume 的六级完整重建、非零实际 BC6H、其余 float ID 实样本与 cube 空间映射保留缺口 |
| STM | Legacy / Dawntrail 模板、染剂索引与行覆盖数据 | `src/stm.rs`、`tests/stm.rs`；借用模板与 Zero/Uniform/Dense/Palette；[格式](stm/README.md) | [两份真实文件](provenance/stm.md)共 86 模板、731 列；185146 个列×Stain 的 half/f32 原位输出共 4443504 字节独立一致，19 块完整提取亦相等。真实材质引用与 129 行 Stain 表有对照；Uniform/Dense 为真实值重编码实验，额外历史表示缺少生产输入或独立规格 |
| IMC | 默认/variant 子集、部件掩码、material/decal/attribute/sound/vfx/animation | `src/imc.rs`、`tests/imc.rs`；零分配借用查询、原始动画字节与低 nibble；[格式](imc/README.md) | [基础样本与装备/配饰扩展](provenance/imc.md)：1812 个命名路径、53690 条记录逐字段独立一致，含82条非零 decal、71条非零 VFX、2条实际动画字节10；全部256动画字节保留/低位分拆有回归。另有[三组武器 VfxId 1/2 资源关联](provenance/binder.md)。decal/material-animation 播放与完整路径回退未验收，不以原始字段对照替代 |
| EQDP | race/sex 模型与材质可用性、稀疏块查找 | `src/eqdp.rs`、`tests/eqdp.rs`；借用稀疏块、缺省零、装备/配饰领域五槽及原始位；[格式](eqdp/README.md) | [十份完整逻辑表、100480 条目](provenance/eqdp.md)，含六份配饰表与全缺省表；十份实际 MDL 的 6132 顶点和全部元数据独立对照，骨名/材质支持左右 ring 槽与跨槽/race 材质共享；两份 low-only 实际 MTRL 全字段已验。material 位完整运行时语义仍有缺口 |
| EQP | 跨槽及身体/耳/尾显示规则的原始位与语义 | `src/eqp.rs`、`tests/eqp.rs`；bitmap/rank、缺省块、五槽声明、参数区域来源位与耦合 coverage 字段；[格式](eqp/README.md) | [完整表与客户端静态证据](provenance/eqp.md)：10239 个位置、28790 个字段值独立一致；指定 exe 的 body→head/hand/leg、leg→foot 来源与 arm/leg/neck attribute 关系已定位；四份 MDL 11269 顶点全字段比较，名称→文件局部位已验。耳/尾/头发完整条件、NPC/特殊角色分支仍有缺口，不计为完整显隐策略 |
| CMP | human.cmp 体型缩放参数 | `src/cmp.rs`、`tests/cmp.rs`；RGBA、八种族/五体型/两 clan 查询、height/tail/bust 范围和严格百分比数学接口；[格式](cmp/README.md) | [完整字段、静态消费和数学对照](provenance/cmp.md)：45568 色、1120 浮点位、560 对范围；指定 exe 的端点、坐标映射及百分比策略已定位，56560 个数学插值结果与独立有理数 oracle 逐位相等。其他颜色区变体、完整 NPC/body 回退、胸围骨空间与姿态应用仍有缺口 |
| SKLB/Havok | 容器变体；骨名、父索引、局部 rest TRS；xyzw quaternion | `src/sklb.rs`、`src/havok`、`tests/havok.rs`；1200/1300 容器、v3 类/对象图、列式数组、参考姿态、animation container 对象关联；[SKLB](sklb/README.md)、[Havok](havok/README.md) | [七份真实文件](provenance/skeleton.md)：12 skeleton、1084 骨、13008 rest 系数位全部一致；父链无环、多个 mapper 侧 skeleton 与主引用已区分；其他 tagfile/容器版本及辅助区完整语义缺少样本证据 |
| PBD | 种族关系、骨名、deform 矩阵 | `src/pbd`、`tests/pbd.rs`；借用主表/连接森林、预算受限无分配祖先路径、真实仿射系数、通用仿射乘积、EPBD/PACK 与内嵌 PBD；[格式](pbd/README.md) | [真实 human.pbd 全字段与静态消费证据](provenance/pbd.md)：5 张表、4215 骨矩阵、50580 浮点系数位；327680 次 ID 查询覆盖 46 次命中/5 次无载荷。1032 个路径下标对与独立 child/sibling BFS 一致，含全部 67 条非空路径。指定 exe 的标量、关系、祖先链有序乘积及一个 G×D 消费点已定位；4215 对实际系数的 139095 个通用数学分量独立比较通过。[标量消费链](provenance/pbd-scalar-consumer.md)关联具名参数与316个VS的湿润度运算，含条件性顶点位置依赖。完整 source/target/bind 空间、逆向变换策略、原始标量物理定义、实际绘制选择和 EPBD 条件仍有缺口，不以 rest-pose 近似替代 |
| PAP | 名字、havok_index、binding、track-to-bone；所需 spline 动画必要解码 | `src/pap.rs`、`src/havok/spline.rs`、`tests/pap.rs`；Win32 v0x00020001、v0 spline、静态/动态/identity TRS、u8/u16、quaternion 0/1/2/5、毫秒采样、容器层 Havok 零对齐；[格式](pap/README.md) | [十个非空包](provenance/animation.md)：十五动画、228 块、5775950 TRS 分量与独立 oracle 一致，完整 binding 映射；210 轨道/173 块大包使用显式数组预算，全部内部边界与末块钳制已验。两轮各 2000 包的有界编码库存没有找到 float tracks/额外类型；该证据缺口与 blendHint 组合保留，时间线只保留原始字节（不执行） |
| AVFX | global、scheduler/timeline、emitter/particle、binder/effector、曲线、纹理路径、内嵌模型、未知块 | `src/avfx{,/assets,/binder}.rs`、对应回归；六类节点、累计条目 wire 顺序/类型、31 种 Data 组合、纹理槽位/曲线、索引范围、绘制/发射几何、可选 binder 属性；[格式](avfx/README.md) | [六份基础及二十份扩展资产](provenance/vfx.md)：两批共 1826 参数、3097 引用；[40 个属性槽与三组武器](provenance/binder.md) 的六个 BPID→owner ElementId→骨名唯一匹配已验。宿主选择优先级、世界姿态合成及纹理播放不作为文件解析证明 |
| ATEX | 纹理 mip/cube/浮点负载与必要像素解码 | 共用 `src/tex` 的 80 字节头、mip/image、RGBA8/RGBA32F，`tests/tex_layouts.rs`；[格式](atex/README.md) | [八份 AVFX 引用的 ATEX](provenance/vfx.md) 均为 type-2，含六面八 mip cube，共 106 图像、371367 像素独立 A8/BC1/BC2/BC3/BC7 对照；三份格式 owner 的完整 AVFX 行亦相等。VFX type-2 均匀10万条首块库存无 float/array/volume 候选，仅作有界负向证据；实际这些 ATEX 变体样本缺口保留 |
| SGB | 家具 MDL 与关联 SGB 依赖发现；可靠结构边界 | `src/sgb.rs`、`tests/sgb.rs`；单 SCN1/单 group 家具 profile，layer/instance 相对偏移表，BG 与 SharedGroup 显式路径、三通道 TRS、未知类型公共头；[格式](sgb/README.md) | [general ID 1–2000 与关联闭包](provenance/sgb.md)：1930 根、2088 文件、15796 实例全公共字段/TRS/已解析路径独立一致；4064 模型、993 collision、288 SharedGroup 引用均可定位，无 profile 超界。九份定向样本的七份目标 MDL（33728 顶点）另有全几何对照；枚举不代表完整家具目录，多 scene/group 无实样本，其他类型负载/行为不作完整场景语义声明 |
| EXH/EXD | sheet 元数据、语言、分页、row/subrow、标量/位布尔/字符串；满足目录与模型关联字段 | `src/excel`、`tests/excel.rs`；共享 parent 字符串池、数据区边界、显式输入/元数据/列行数/工作/字符串预算；[格式](excel/README.md) | [七表、九页实资源](provenance/excel.md)：Item 三页、ModelChara/HousingFurniture/EquipSlotCategory 与三种 subrow 表；26668 行、448949 字段、6193 字符串逐项相等；真实 subrow 共享池和 u64 高位已验。另有 [Stain](provenance/stm.md) 129 行；外部列名 schema 与跨表业务装配不作为文件解析声明 |

## 验证范围

[实资源变异安全验证](provenance/input-safety.md)对29个固定种子、17类解析
入口执行18123次原件/截断/字段替换/XOR尝试，包含受预算限制的TEX、SKLB、
PAP及MTRL后续操作，没有panic。该回归不替代独立格式/像素证据，不证明
穷尽非法输入，不将被接受的变异文件作为新的生产变体或解码成功。

[完整volume主索引覆盖](provenance/tex.md#volume-完整主索引覆盖与-index2-对照)
检查59张表全部1802972条记录，补扫226699条后volume样本/字节/像素结果
保持相同，无完整counterpart。[index2碰撞对照](provenance/sqpack-index2.md)
发现31个真实flagged hash和62条完整路径记录；展开后与主索引位置多重集
相等，无额外位置。`Index::parse`和异步加载保留辅助原始记录，完整路径
查询支持index2名称消歧；全部59容器的主/辅助字节与62条位置查询对照
通过。仅提供index2的后端成功提取62条路径，输出与独立主表位置直接提取
一致；该比较复用DAT解码器，不另计为独立像素/模型解码证据。

[PBD输入来源与仿射逆](provenance/pbd.md#输入矩阵来源与独立求逆数学)确认
`G`读取有有效位缓存与逐part资源矩阵两个分支，资源初始化另有referencePose/
parentIndices→矩阵→求逆的静态线索；指针绑定和完整空间仍未闭合。
`Matrix::try_inverse`独立使用消元，不选race/EPBD路由；全部4215实矩阵的
50580个逆系数与精确有理数oracle一致。数学逆不代替反向race变换验收。

[Part 资源绑定与缓存输入](provenance/pbd.md#part-资源绑定与缓存输入)将
三个非零setter直接调用的输入与SKLB请求、factory及SkeletonResourceHandle
类型/初始化关联；+0x58表项来源有指定路径的字节证据。另有两个已识别的
缓存写入直接调用，其输入包含另一记录的仿射逆。29个静态区间与27条调用
边复验不代表运行期选择、全部别名/写入者或完整PBD坐标空间已经验收。

同页的[条件上下文输入](provenance/pbd.md#条件上下文的-referencepose-输入)
另追到所选SKLB skeleton的referencePose复制、parent处理和stride 0x70记录
生成；它只证实初始化来源，不等同于活动动画姿态或完整坐标命名。
[父累积与记录矩阵](provenance/pbd.md#参考姿态的父累积与记录矩阵)有独立静态审核：
五旧窗1304字节、两callee1661唯一字节及两数据区20字节全部对回原件。
单位四元数下，父平移段识别为`R(q_p)t_l+t_p`，记录十二分量识别为
`[R(q)diag(s) | t]`；局部未乘父scale不排除中段修正。四个W槽、导出寄存器
来源在五窗内的限制、状态/别名及父输出就绪条件分别保留；这些事实不代替
完整source/target/bind空间或binary32执行验收。
[条件记录的生成与导出来源](provenance/pbd.md#条件记录的生成与导出来源)以
含3100字节中段的连续窗口确认：顺序导出前有四元数四lane同源赋值；
另一关联记录分支明确先以scale乘local translation。B的T/Q来自A，S另取
pose条件路径；三种A来源、104/112字节copy-forward、40字节返回TRS读取
及ABI/容量限定经过独立审核。未知常量、helper和窗口外分支仍影响完整
空间解释；共享累计4761/8192新代码字节、3/4入口、2/2数据区、零查询。
[有值分支的仿射乘积](provenance/pbd.md#有值分支的仿射乘积与-w)逐项核对
helper十二个输出：列向量约定下`O=R×L`，相应调用为`A[d]=D×F`。
两个输入与输出各访问十二个DWORD，按矩阵基址不访问四个W槽；输入读取
全部早于输出写入，错位别名/栈/ABI及下游W使用限定保留。该局部乘积
不替代全空间身份，21–23共享累计4/4入口、5629/8192新代码字节，数据区
数量仍为2/2，零查询；入口及数据区额度耗尽。
[参考姿态差值假设](provenance/pbd.md#四种参考姿态差值假设的实数据核对)
在三个实际parent/child对、308个同名骨上比较四种标准TRS矩阵差值，共
14784系数，四种公式均有反例。该库存/假设观察不算空间语义完成，也不
把参考姿态差值写成PBD数据的替代实现。

[Cube 面序与类型化视图](provenance/tex.md#cube-面序与类型化视图)将固定Win32
初始化分支的描述索引与已审查Microsoft API契约关联，六个文件ordinal对应
Direct3D的+X/-X/+Y/-Y/+Z/-Z。两个非零cube的60张encoded图像与独立切片
字节一致，两个base mip均有六个不同面。CubeFace视图不旋转/翻转像素，
不推断环境图世界轴、面内UV采样公式或cube-array头部变体。
矩阵中保留的cube空间方向边界指面内UV与场景坐标映射；六面ordinal的
Direct3D命名与这些消费端坐标问题分别记录。

[EPBD 根标量与局部 selector](provenance/pbd.md#epbd-根标量与局部-selector)的
native 验证/加载路径确认根字段0/1为u16标量，按两字节宽度校验，显式值与
省略可区分。八个实际 selector 均匹配内嵌 skeleton ID、其祖先包含
entry parent；API 保留原u32并暴露低u16视图。五个仅修改根字段的受控副本
与原件均完成全部字段/矩阵对照；它们不冒称生产文件，也不代替完整选择策略。
同页的[角色模型槽请求来源](provenance/pbd.md#角色模型槽请求来源与精确-pair-查询)
另确认第一外层ID来自角色，第二ID与selector来自所选模型槽记录。原始
child/parent精确查询有预算、重复匹配歧义及独立对照，不应用原生回退。

[PBD 标量的具名消费](provenance/pbd-scalar-consumer.md)将比值首f32写入、
`g_ModelParameter`动态名字索引、同源TLS context与packet数据指针关联。
七份SHPK的8589个shader资源表中，316个VS均读取`m_Params.x`，进入具名
`m_Wetness`的MAD→MUL→MAX→MIN链；252个直接使用POSITION0.y，64个使用
临时位置分量。character VS2输出TEXCOORD4.w；VS48使用法线偏移后的位置，
湿润结果参与风向量衰减并条件性到达SV_POSITION。该静态证据覆盖具名运算
和两条下游路径；原始scalar的物理单位、实际draw选择/绑定和完整PBD空间
仍有独立证据缺口。

[R16F / R11G11B10F](provenance/tex.md#r16f-与-r11g11b10f-的映射和数值证据) 的
客户端 ID 映射与独立 Microsoft CPU 数值对照已固定；两种格式的 2D/array/
cube/volume 共 291456 个 f32 分量，有限值/零/无穷逐位一致，NaN 类别一致，
RGBA8 全像素一致。它们有可用 CPU 实现与受控回归，实际游戏纹理样本仍有缺口。

[BC4 / BC5](provenance/tex.md#bc4-bc5-的实际纹理与端点语料) 的四份实际 TEX
包含 38 张 mip 图像、699050 个像素，全部 type-4 字节和 RGBA8 像素与独立
观察一致；每个通道命中两种端点分支及全部 selector。另有每通道全部 65536
个端点对、八种 selector 的受控语料，共 2097152 个像素逐字节一致。固定
客户端将两种 ID 映射至 BC4_UNORM / BC5_UNORM，不按 SNORM 解释。

[Glass mesh 有界调查](provenance/mdl.md#glass-mesh-实样本调查)检查 21 个 bgcommon/bg
index 的 500000 条记录及 78581 个 runtime 区，未找到非空 Glass 范围。
该库存观察不算完整几何回归，不解除 Glass 实样本缺口，也不证明安装中不存在。

MDL 的[声明身份受控实验](provenance/mdl.md#声明身份与便捷查询的受控验证)在实际
2007 顶点的 payload 上建立七份仅修改 usage/index 的副本，合计八种情形、
138552 条查询独立一致。不同 usage-index 分开解码；重复语义、不完整 skin
pair 和非法 vertex 显式报错，原始声明保留。该实验不冒称取得实际多 index
游戏文件，也不解除非空 Glass 等实样本边界。

[Color1 / 双颜色实际文件](provenance/mdl.md#color1-与同-usage-多-index-的实际文件)
另有四份 MDL，全部 type-3 重建和几何对照通过，含 28 个 LOD/mesh、13256 个
顶点记录、35799 个 indices 和 104273 条语义查询。四个 mesh 只有 color1，
六个同时有 color0/color1；4854 个顶点的两组值不同。颜色多 index 的实样本
证据已固定，normal1/bitangent1/flow1 的实样本边界分别保留；usage 5 的
flow0 输入身份见下述客户端独立关联。

[UV 高 index / 混合 scalar](provenance/mdl.md#uv-高-index-与混合-scalar-type-的实际文件)
另有四份 MDL，51 个 LOD/mesh、91404 个顶点记录、211863 个 indices 和
735856 条查询独立一致。实际 UV index 0/1/2 使用 half4/half4/half2，另有
half2 的 index 0 与 normalized byte4 的 index 5 并存；没有将其猜测为 flow。

[客户端顶点输入关联](provenance/vertex-inputs.md)独立确认 raw usage 5 →
内部编号14 → TANGENT0，usage 6 → 15 → BINORMAL0。上述实际 MDL 中另有
3926 条 usage5/index0 查询逐位一致。七份 SHPK 的588个 vertex shader、
4490条 ISGN 记录中有72个 TANGENT0 声明（68个非零读取 mask），支持其
tangent/flow 编码输入身份，不作为具体流向公式或 model/permutation 绑定证据。
指定转换路径对 normal/usage5/usage6 忽略 raw index，不能将任意非零 index
称为原生 normal1/flow1/binormal1；API 仍保留原始声明身份。

[Float1 / Float2 兼容身份及查询](provenance/mdl.md#binary32-布局与位保留验证)
将 raw type 0/1 经独立客户端表项与 Microsoft 规格对应为 4/8 字节 binary32。
八份受控模型包含每种 type 的 2048 种位模式，48192 条完整查询、每侧
2189672 字节一致；固定 C0 的全几何结果与既有登记相同。短输入、stride
越界和输出预算有回归；type 0/1/3 的实际生产模型样本边界保留。

[Legacy 材质调查](provenance/materials.md#角色主索引的-legacy-材质调查)选择角色主索引的
全部 334633 条记录，检查 196873 个符合界限的 type-2 首块，没有 544 字节
dataset 候选。另对全部 322 份实际 512 字节 dataset 材质完成独立逐字段对照，
含七个 shader 家族、5152 行及 15543 个 HDR half 值。这补充 Legacy 颜色表
的实际样本覆盖，不作为实际 Legacy dye 或完整染色求值的证据。

PAP 另有[真实零记录空包](provenance/animation.md)证据：没有 Havok 对象流，
仅六字节对齐；API 显式返回缺少 tagfile，不创建默认动画。

[BC6H 多模式 CPU 对照](provenance/bc6h.md#microsoft-cpu-多模式-oracle)另覆盖
14 个有效模式与 4 个保留前缀的 4608 个受控块，294912 个 RGBA32F 分量与
固定 Microsoft DirectXTex oracle 逐位一致。它消除的是其他模式缺少独立数值
对照的缺口，不证明非零真实游戏样本或穷尽所有块位模式。

两份 volume 的[离线压缩块调度证据](provenance/tex.md#压缩块调度与-volume-目标跨度)
固定了描述符编号 6 到单块助手的间接链、按 expanded size 推进的块目标与按
切片大小乘深度推进的 mip 目标。[分配器路径](provenance/tex.md#volume-上传缓冲区的分配路径)
另固定了构造器安装的堆链及已有区域子分配：只有元数据写入，没有 payload
初始化，不能由分配行为假定缺口全零。实际路径选择和差额字节来源未证实；
静态链通过不计为完整重建通过，1888 / 2488 字节差额保持明确失败，不补零。
六组受控回归同时核对真实尺寸/长度形状的失败与完整 mip 字节对照的成功。

[扩大 volume 验证](provenance/tex.md#volume-库存-重复-offset-与完整像素)对选中索引中的
482 条 volume 检查全部压缩组。209 条完整记录、1940 图像、2370564 像素独立
一致；其中四个完整头部全部有效 offset 为80，提供显式 packed-volume 读取，
不改变默认严格 offset 检查。另273条（含两份固定见证）仍有1888/2488字节
差额，虽全部吻合 double-depth-shift 长度轮廓，缺失字节值仍未证实，完整文件
读取保持失败。

[完整单 mip 读取](provenance/volume-mips.md)让这 273 条的 base mip 均可用，
保留原六级头和 mip ordinal，覆盖 7728 切片、1978368 像素。短资源共有
672 个完整 mip，加完整对照 6 个，公共 SqPack→TEX API 的 7917968 RGBA
字节与独立 zlib 输出一致；966 个不完整 mip 精确报长度错误。272 条路径的
天空加载/texture转交/effect/sampler链支持初始化 MinLOD=MaxLOD=0；
`sky_400` 不在 native 天空表内。该静态状态不扩展为全生命周期或六级完整
重建，单 mip 成功也不验证未请求组的压缩流。

[counterpart 与初始化描述检查](provenance/tex.md#volume-对照条目与初始化描述)
将273个缺口分成177个base-mip组，选中完整条目中无同尺寸/格式/base摘要对照。
59张主index表的完整记录中无flagged或重复hash，不扩大为全DAT覆盖。
静态初始化链对BGRA8仍按完整mip深度推进source，清零对象是栈上描述表，
没有据此建立像素缺口的补零或重建规则。

[物理存储与创建/复制证据](provenance/volume-generation.md)覆盖273条到
下一索引条目的完整区间：自身分配区316347字节未消费余量全零，额外空间
为四条无像素载荷的type=0记录。固定创建链推导出IMMUTABLE、仅SRV绑定、
MiscFlags=0，不满足自动mip生成条件；替换command 11复制已有subresource。
这些范围与状态没有提供缺失像素，六级完整重建数为0。

同页的[像素关系](provenance/volume-generation.md#已存像素的三维滤波关系)
强支持三维mip前缀：273条529116个已存低mip字节均落入周期线性插值积分的
floor/ceil区间，数值z步长为2^m。加Bayer量化后177条去重链的277461个BGR
通道仍有481个±1反例，其中同邻域/同相位的不同输出及非tie反例阻止精确
恢复；生产精度、运算顺序与前缀写出路径需要独立代码证据。

[原源码 CPU 探针](provenance/volume-generator-oracle.md)分别复现同邻域
200/199 和非 tie B=51 的浮点机制；固定奇数行相位适配后的四条见证 mip 1
共4096字节一致。DirectXTex原始递归浮点加抖动路径仍有725/4528字节不同，
完整统一生成器与生产前缀写出链未确定，六级完整重建数为0。

同页的[直接 base 总体](provenance/volume-generator-oracle.md#直接-base-张量的目录总体)
包含正向天空目录的273短条目及179完整对照：591408个已存低mip字节中，
separate/FMA候选分别有131/113个差异，短条目为89/79。原始SSE归约与
Python的431196个float逐位一致；独立块解码复核452个entry和2712个组。
等候选输入的不同实际输出排除仅改逐点量化；FMA另有两个非tie反例。
[源域检查](provenance/volume-generation.md#缩短源域的条件性反例)复现7个
crop-local wrap/clamp反例；没有区分“完整生成再截前缀”与“只计算前缀坐标”。
DLL字面量未建立D3DX调用身份，缺失603624字节及六级完整重建0/273保持明确。

[Scalar/x87 门槛](provenance/volume-generator-oracle.md#scalar-与-x87-的实际精度)
在八个固定见证所在行比较116 byte：自动向量化的SSE/scalar混合路径差2，
保留x87累计的路径差7；均停止扩大。每份构建的163840个归一化标量、116个
中间float及116个最终byte经整数有理数逐位复核。三版官方历史源码的抖动
修复没有改变物理x相位；223366个坐标通道的源贡献序列检查得到每种26个
有向匹配（13对），无输出冲突，不构成生成器证明。
[FIIN元数据](provenance/volume-generation.md#fiin-元数据入口)仅取得与具名
DLSS松散文件相符的一条长度/SHA-1记录，没有目标TEX的生产writer信息。
04/05证据已封存；统一生成规则、生产调用及前缀写出链仍需独立验收。

[天空作者输入与坐标链](provenance/volume-production.md)固定四个2019 PSD
窗口名字，其中064与2026同名base颜色明显不同；未建立PSD字节或精确尺寸。
147条depth32的z24…31等于z0…7，1204224字节零差异，452条身份经独立
解包复核。表u16=24进入CPU的`N×t/D+0.5/D`，t来自输入除以86400，
Sky/Sky2直接将结果作为第三维采样坐标；压槽分支不交换表ID，须保留条件。
2018构建讲演正文需登录，2014 SqPack流程未提供mip/writer细节。
06的作者、base和消费证据不改变完整重建0/273及603624缺失字节。

[天空 CPU 投影与缓存链](provenance/volume-sky-consumer.md)确认 BGRA8 的对象
mip 0 采样、27-float 九基累加和近似四倍实球谐的数学识别。新建天空请求
经具名 TEX factory 将 bit 1 带入 mode 1，已有 handle 受不同条件约束。
缓存复制长度为 `u32(S_m×D)`，源推进为 `u32(S_m×max(D>>(m&31),1))`；
表非空不独立证明输入来源。运行期入口、生产生成及前缀 writer
仍有证据缺口，07 静态链与归档检查不计为缺失 mip 重建。

[环境方向与 AMB](provenance/volume-environment.md)确认 p 的角度来源及理想
单位大圆形式；27float的时间结果F与方向处理副本P具有独立字段和用途。
实际skylight的51块、806544字节、592目录、7424记录经独立解包核对，
ID/count与天空表逐项一致；451条cohort关联及141条cohort外分类分别记录。
两种AMB序列化链到达具名文件IO导入，但实际生产调用、统一mip生成器及
TEX前缀writer未确立。完整重建0/273、603624缺失字节不以此替代验收。

同页的[零阶系数必要条件](provenance/volume-environment.md#base-与零阶系数的必要条件)
用451条base与6708条AMB记录作方向无关的颜色区间比较，独立复算20124个
通道的全部界与原bits。不平方分支在441条资源上被排除，平方分支全部
位于界内；开关未知时仍是有界非排除，不能据此确立生产来源或恢复低mip。

同页的[文件打开调用者](provenance/volume-environment.md#文件打开入口的调用者)
沿确认的helper一次查询16个call/11个caller，正向识别24-bit BMP导出与
256条固定编码表写出。该范围没有TEX前缀或逐mip步进证据，不表示全局
不存在其他writer；09的148文件封存与核验不改变重建验收数。

[Volume装载与块任务](provenance/volume-block-loading.md)的10独立审核覆盖
45槽、23个override及30个新入口。完整mip目的步长与组内块输出长度分开，
末尾两项检查不验证块输出之和等于mip容量。11/12主调查绑定任务到回调的
源/目的参数，并确认长度槽更新条件与返回值未检查；其语义没有独立审核，
实际分支、间隔内容、生产生成器与前缀writer仍需各自证据。
同页的[分配器写入范围](provenance/volume-block-loading.md#分配器与元数据写入)
以13主验证绑定已有区域的子分配、成功后的排序helper和新区域初始化。
元数据不别名等前提保留，上层虚分配器及实际payload内容未知；分配语义
不提高六级重建验收数。

[命令引用与条件复制](provenance/volume-block-loading.md#命令引用与条件复制)
以14主验证绑定type11字段、16-byte引用整理、双缓冲lane选择与递归参数。
发布lane0到根lane30的可达与COM来源保留缺口。15的
[创建输出与条件注册](provenance/volume-block-loading.md#创建输出与条件注册)
绑定同一创建参数的输出地址、条件性TLS赋值及wrapper接口赋值；owner与
全局/batch的身份、所有线程注册和lane首块准备另有边界。复制参数与引用
传递不构成生产生成器、相位或TEX前缀writer证据。

EQP 的[Head 三位 hair code](provenance/eqp.md#head-hair-三位字段与耳部关联)已从
固定客户端的读取、mask 与跳转表确认，2879 个实际存储记录及全部 256 个低
字节受控值与独立字段观察一致。API 保留 code 0–7；部分耳位另有与
`atr_mim/atr_top/atr_hrn` 的静态关联。完整耳/尾/头发条件及 NPC 状态仍未验收，
不将单个位 getter 或 raw code 当作完整角色显隐求值。

[耳/尾槽位与字段门控](provenance/eqp.md#耳与尾的槽位-启用条件及实文件关联)
另确认普通tail分支的Body/Leg enabled门控、类型8复用槽位为zear并使用
Head/角色状态的区别。四份tail/zear/face资源完整type-3/MDL对照覆盖24107
顶点、60次局部attribute查询；EQP额外37427个声明字段值一致。该项解释
参数条件及名字局部性，不引入显隐状态机，也不替代NPC/其他覆盖分支验收。

[MTRL染色覆盖profile](provenance/materials.md#染色覆盖-profile-与中间行)区分
源ColorLayout与中间行覆盖目标：五项/十二项profile由shader身份选择，
投影API要求显式选择，不从32行表猜测。155648次受控投影及两个实际材质64行的
128次投影对照通过。通道门控与specular全零保留有局部静态证据；完整染色
写入、Legacy颜色转换与shader数值求值仍不由声明掩码代替。

`mtrl::dye`的显式half行覆盖另有[完整结果对照](provenance/materials.md#显式-half-行覆盖的独立数值对照)：
不可变32-half行、模板精确查询、原始位复制、通道门控、specular全零保留和
实际写入掩码。62624个请求逐位/逐结果类别一致，其中32246行输出、128跳过，
其余为明确的缺模板/非法输入结果，不计为染色成功。86个实际STM模板及
64条实际材质行与受控flags分别标注；这是单行操作的独立回归。

[Legacy 色行转换与整表染色](provenance/legacy-color-conversion.md#完整色表输出与染色关联)
提供不可变32×32-half基础表及完整输出。32个目标lane、16→32行首行扩展、
Legacy dye只作用前16行、shader/profile、双通道、stain零/reset和缺模板保留
均有静态路径依据。全65536个half输入与独立数学oracle一致，包括1024个
binary32中间舍入见证；数值契约为nearest-even减法再half向零截断。
5620个整表请求逐字节比较基础/最终2048字节及写入掩码；322份真实Legacy
输出、两份实际32行dye的510次请求和受控语料分别计数。真实源lane3为20
的行在库输出中得到−19。实际544字节资源、运行期上传、任意MXCSR状态和
shader像素效果不由这些数值回归代表。

- 安装来源：用户授权的只读 `~/Games/ffxiv/game`，Win32 SqPack。
- `ffxivgame.ver`：`2026.10.06.0000.0000`。版本文本本身不证明所有包版本一致。
- 不覆盖所有历史平台或版本。每项验证需列出实际样本与覆盖的变体。
- 基础检查：`just check-all`（包括 WASM），文档：`nix develop -c notist build`。
- 固定实资源联跑：`tools/verify_render_resources.py`，命令与报告规则见[回归入口](provenance/README.md#资源回归入口)；通过回归不等于消除本表缺口，已知错误复现单独分类。
- 不启动游戏、不附加进程、不运行服务器/GPU 测试、不修改安装。
- EST、TMB、GPU 渲染、角色装配、VFX 模拟、应用集成排除在此验收范围外。

## 需求来源

需求读取范围为 xiv-companion 的 `docs/render-book/{resources,geometry,materials,model-routing,skeleton-animation,vfx-assets}.not`；这些文件用于确定消费语义，不作为解析代码模板。
格式事实、许可审查、样本身份与可复现实验见 [来源与证据](provenance/README.md)。

## Legacy 解析层对照

[隔离功能审计](provenance/legacy-coverage.md)确认常用解析域具有活动库对应
接口；MDL 单/双 binary32 的解码差项有独立依据与受控验收。少量历史
STM/MDL/SKLB 输入兼容仍是候选，未找到本次需求必须读取它们的生产见证。
PBD 完整空间与 EQP 全部显隐也不是 legacy 已独立完整验收的能力。
这些区别不阻止按已登记实资源接入常用解析，但不能据此宣称旧调用或
整个 legacy 已全部替代；完整渲染需求仍按下述判据逐项验收。

## 验收判据与输入边界

验收对象是从资源字节取得可消费的数据。文件定位、原始字段保留、必要解码、
语义映射和应用策略分别判断；其中一层通过不能替代其他层的证据。
固定资源联跑的通过条件不包含整个需求矩阵的完成判定。

### 阻止完整资源解码验收的见证

SqPack/type-4 的 273 个 volume 条目有明确的输入/输出长度不一致，包含
`ffxiv/010000.win32.dat1@967239424` 和
`ffxiv/010000.win32.dat0@1609282304` 两份固定见证。其完整块解压输出分别
比声明总长少 1888 和 2488 字节。全部压缩块及 mip 组已经检查，不是只读取
首块所得的判断。完整 profile 的解析、更多同长度族见证和精确长度公式
均不能确定未提供字节的值。

这些条目的 [672 个完整单 mip](provenance/volume-mips.md)具有可消费的真实
像素，包括全部 273 个 base mip；所选 mip 的全切片长度、原始身份与预算
均独立核对。此处阻塞的是六级完整重建，单 mip 输出的正向验收单独成立。

[Meddle / Physis 审阅](provenance/external-volume-review.md)另区分了工具的
宽松返回行为：Physis 顺序拼接短输出，Meddle 顺序写入声明长度的零数组。
这两条指定版本的路径都没有提供缺失像素值的证据；尾部零也不等于各 mip
槽内缺口的内容。本地块边界复验与外部实现观察分别记录。
[TexTools PR #96 审阅](provenance/volume-mips.md)
也没有给出 volume 缺失切片来源，固定许可证与接触范围保留。

允许的替代观察包括独立 raw DEFLATE、482 条有界库存、209 条完整像素对照、
客户端调度/目标跨度与分配链静态检查。已有区域子分配没有 payload 初始化，
因此也没有以堆清零为依据的确定性补全规则。GNU objdump 的静态路径不证明
具体条目的运行路径或上传内容；REA 原生引擎不可用不由静态替代工具冒充。
完整counterpart比较及staging→逐mip描述→volume创建参数的数据流检查也未
给出差额字节；创建描述区清零不等于像素初始化。这些负向证据分别固定，
不提升为 273 个资源的六级完整重建成功。

到后继索引条目的[物理字节记账及创建/复制路径](provenance/volume-generation.md)
排除了该范围内额外已编码块、尾随流及该创建状态的自动GenerateMips；
物理填充全零和复制已有subresource均没有定义缺口内容。
三维积分前缀与Bayer量化的数值关系通过独立精确有理数复算，但已存字节
仍有明确反例；候选模型在缺失位置的输出不能认证为生产像素。

解除这项障碍需要以下任一可独立核对的输入：

- 同版本/平台、可定位到这些条目的完整生产输出与 mip/slice 边界说明；
- 能解释省略区内容及适用条件的生产格式规范或资源生成实验；
- 合法取得、带条目身份和初始化/写入边界的独立运行观察，用于确定差额
  是否具有可复现内容。仅有一次缓冲区快照不足以证明通用补全规则。

这些输入均需附版本、来源及使用许可。它们不是对执行游戏、附加进程、GPU
测试或修改安装的授权；本任务不为取得输入而执行上述操作。

### 语义证据与变体证据

[全主索引 type-4 头检查](provenance/tex.md#全主索引-type-4-纹理头覆盖)
覆盖59张表全部1802972记录与405685个type-4头，包含逐表抽样之外的45252个
头。只有既有0x2460与零BC6H cube两份浮点/BC6H见证，没有增加其他float
或非零实际BC6H样本。该负向结果不覆盖type-2 TEX/ATEX，不计为新增像素解码。

[全主索引 type-2 首块检查](provenance/tex.md#全主索引-type-2-tex-头候选覆盖)
另覆盖同一批1802972记录，检查1184882个合格首块、解压9665818722字节，
取得227828个TEX候选头；没有符合条件的float/BC6H/array/volume头。
VFX分类外的ATEX名称有实际index2记录证据，搜索不局限08分类。这个库存
不排除分割头/超出声明预算/未索引数据，不代替完整像素验证，也不取消样本边界。

- MDL `geometry.not` 明列 normal1/bitangent1/flow0/flow1 为可选输入。
  `Geometry::attribute(vertex, usage, usage_index)` 已验证按原始声明读取，
  usage5/index0 的 TANGENT0 身份有客户端关联与实际数据证据；高 index UV
  和颜色不能证明其他 flow 映射。normal1/bitangent1/flow1 需要带已知输入
  语义的声明/负载实例或独立的消费端关联。非空 Glass
  分类也需真实实例或独立结构观察；空范围与自行构造分类只验证查询机制。
- MTRL 的 Legacy dye 有静态布局和全 u16 受控证据，实际 544 字节材质缺少
  见证。TEX 的非零实际 BC6H、部分 float ID，以及 ATEX 的 float/array/volume
  也是实文件覆盖边界；它们不抹去通用 CPU oracle 的数值证据，也不能被
  受控语料冒称为实际游戏资源。需要带身份的对应资源扩充实文件覆盖。
- PAP 的必要 transform spline 解码具有实际采样证据。非零 float tracks 在
  `Tagfile::spline_animation` 中明确拒绝；两轮各 2000 包的负向调查不证明
  需求资源不含它们。若目标包包含这种轨道，需该包与独立数值证据才能扩展
  解码，不将返回错误计为采样通过。blendHint 的原始值不等于组合规则。
- SHPK hull/domain 非空记录、SGB 多 scene/group、其他 SKLB/tagfile 版本是
  指定实样本之外的变体边界。目标范围不要求穷尽历史版本，但也不据有界
  负向搜索宣称这些变体不存在或已验收。

### 应用层消费与底层语义

角色装配、候选路径回退、姿态/蒙皮矩阵计算、VFX 模拟及 shader 执行属于
排除项。CMP 参数与百分比数学、PBD 森林与仿射系数、EQP 参数位和区域来源、
MTRL/STM 表及覆盖声明则属于本库的数据输出。

完整 NPC/body 回退、胸围姿态应用、材质播放和 Binder 世界姿态不作为底层
解析的完成条件。反之，PBD 矩阵的 source/target/bind 空间解释、EQP 耳/尾/
头发字段的条件意义、染色覆盖字段的含义不能仅因消费发生在上层就宣称
证实。它们在各自来源记录中保留为语义证据缺口；通用仿射乘积、raw bit
getter 和原始表格对照不替代完整原生策略，也不引入近似策略作为解析事实。

带输入输出身份的独立消费观察或可审查来源的格式说明，可用于澄清上述
字段语义；应用状态机和最终装配实现不因此扩入本任务。缺少这些证据时，
输出只承诺已验证的原始字段和数学操作，不承诺完整角色效果。
