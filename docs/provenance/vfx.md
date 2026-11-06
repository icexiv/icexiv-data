# AVFX / ATEX 来源与验证

## 需求、来源与许可

需求来自 xiv-companion commit `a38d3c9bfb67fb441d17a77b344f77c83058dca0` 的
`docs/render-book/vfx-assets.not`，只读取消费语义文档，没有阅读其解析实现。
没有读取本项目 legacy，没有引入现成 FFXIV 解析库，不宣称严格 clean-room。

0ceal0t/Dalamud-VFXEditor commit `ed6b39dcca04225c15c404a35a46945119462015`：
本轮先重新获取根 LICENSE，确认为 MIT、Copyright 2021 Michael Kaminsky，再阅读
`VFXEditor/Formats/AvfxFormat/Base/AvfxBase.cs`、`Model/AvfxVertexes.cs`、
`Model/Emit/AvfxEmitVertexes.cs`、`Curve/AvfxCurveKey.cs` 全文。
阅读包含构造、读写、UI 和编辑逻辑；采纳的是块名称/长度、顶点字段与关键帧字段
的公开结构结论，未复制、翻译或照搬代码，没有执行该插件。

另访问 [XIV Dev Wiki AVFX Files](https://xiv.dev/game-data/visual-effects/avfx-files)
的公开格式页（页面未给出明确内容许可证），只作结构索引与交叉检查，不复制其
文本、代码或图作为本项目文档。页面先于本轮源码阅读被工具展开；不据此声称
所有资料均在看到任何内容之前完成许可证审查。采用字段以已审查 MIT 声明和
下面的真实字节观察为依据。页面的四个 UV half 分量 × 两组与源码的二分量 ×
四对同占 16 字节，保留八个系数，不凭两种描述决定运行时的 UV 选择。

ATEX 使用公共 TEX 解析器；通用 Pillow 许可审查见 [TEX](tex.md)，本轮验证环境
为 Pillow 12.2.0。Pillow 只解码标准 DDS BC1/BC3，不包含 FFXIV/Havok 解析器，
不作为 Rust 库依赖。

## 实际 AVFX 观察

授权只读 Win32 安装版本 `2026.10.06.0000.0000`。
`tools/find_vfx_samples.py` 对 base vfx/chara index 每包最多 100000 条记录进行
有界扫描，检查标准文件首块签名；本轮检查 301 个候选就取得六份 AVFX。
文件以 `tests/fixtures/avfx-observations.json` 的 DAT 路径、绝对条目偏移、index
ordinal/hash 与提取字节 SHA-256 固定；未反推或声称知道其原始 AVFX 资源路径。
纹理路径是这些实际文件的 Tex 负载，不是猜测的文件名。

六份文件的 Ver 均为 `0x20110913`，共 140 个顶层节点、60 条纹理路径、38 个内嵌模型。
独立 Python struct 逐项对照 30614 个节点子块的边界/对齐、2673 个 key、17 个
Clip、4263 条绘制顶点、18399 个索引、236 条发射顶点与编号。绘制顶点 half
由 Python struct 的通用 `e` 解码，不调用 Rust；所有位置/UV/normal/tangent/color
输出与 Rust 相等。完整 scheduler 最后累计记录与时间线 emitter/binder/effector
引用也逐项对照；负索引保留原值，不赋予未经验证的宿主语义。

独立观察发现 Life/CrI/CrIR 的同名负载有四字节标量与子块两种变体；递归观察
工具按此区别展开，库只在调用方明确要求时解释子块流。Name 字符串即使字节
碰巧能组成块头，也不会作为结构证据。已知数值与几何之外的未知负载原样借用。

`tools/verify_avfx.py` 独立重建全部 type-2 文件字节，和 Rust SqPack 提取比较，
再和 `examples/validate_avfx.rs` 的全字段语义行比较。文件、模型和完整报告不放入
仓库；样本身份和结果行哈希保留。通用块图不等于所有 emitter/particle/binder/
effector 类型的类型化 Data、引用约束和播放语义已验收，覆盖清单保留这些缺口。

## Data 类型和引用对照

在相同已审查 MIT commit 下，重新取得 LICENSE 后又阅读以下内容：

- `Base/Enums.cs` 的 EmitterType/ParticleType/BinderType/EffectorType 数值声明；
- `Emitter/AvfxEmitter.cs`、`Particle/AvfxParticle.cs`、`Binder/AvfxBinder.cs`、
  `Emitter/AvfxEmitterItem.cs`、`Emitter/AvfxEmitterItemContainer.cs` 的字段声明，
  截止各构造方法之前；
- `Emitter/Data/AvfxEmitterData{Model,SphereModel,CylinderModel}.cs`、
  `Particle/Data/AvfxParticleData{Model,ModelSkin,LightModel,Powder,Disc,Windmill}.cs`、
  `Effector/Data/AvfxEffectorDataPointLight.cs`、`Binder/Data/AvfxBinderDataPoint.cs`
  的字段声明，截止各构造方法之前；
- `Particle/Texture/AvfxParticleTexture{Color1,Color2}.cs` 的字段声明；
  `Components/IntList/AvfxIntList.cs` 全文，包括 byte 读写和 UI 方法。

`Emitter/AvfxEmitterIterationItem.cs` 请求为 404，不作为已读资料。
只阅读声明而没有实样本的类型不自动计入验证范围；ModelSkin 的实样本见扩展变体节。
未复制或翻译来源解析方法，未采用 UI 的默认值补缺省字段。

`tools/verify_avfx_assets.py` 用另一套 struct/字典观察，对同六份实际文件的
十种 Data 组合逐项核对类型判别、498 个已知参数、86 条最终累计 emitter item、
438 个纹理槽位及 852 条静态引用。类型覆盖为 emitter sphere/cylinder/model，
particle powder/windmill/model/disc/light-model，point light effector 与 point binder。
曲线、曲线组和未知字段分别输出，不把曲线组里的 ACT/ACTR 当成固定轴值。
Binder 的 BnVr 数值也独立加入基础节点对照，基础行哈希与资产行哈希分开固定。

观察确认 emitter MdNo 是 i32，particle-model MdNo 是一字节模型列表，实际
light-model MNO 却为四字节整数，与所读源码的默认一字节参数存在差别。
实现按实际宽度保留 byte/i32 表示；一字节 MNO 与多元素 MdNo/TLst 用受控
wire 实验验证，不冒充游戏文件。粉末 bLgt 的一字节形式亦由实样本确认。

852 条范围观察中 424 条位于目标数组内，427 条为负值，1 条非负越界。
越界值来自样本 #2、Ptcl 绝对偏移 46012、TP 的 TxNo 字段偏移 49208：
值为 26，bEna=0，根 Tex 数量为 9。它是被禁用槽位的保留值，不能据此将整份
真实资源判为不可解析，也不能从一次禁用状态推广所有越界索引的运行时含义。
负值不被无条件解释为“确认没有引用”。另有一条 TxN 动画选纹曲线保留为完整
曲线，不把其数值强制作为一条静态引用。

受控测试覆盖条目外层标签与累计末记录的对应、多个 byte 模型索引、负引用、
禁用槽位的越界值、未知 Data 原样保留、显式输出/块/数量/工作预算，以及
4096 次确定性变异。`AssetReport` 是已解码字段的范围报告；未知 Data/字段和
未建模的播放策略仍需单独验收，不作为“所有引用已完整验证”的声明。

```sh
nix develop -c cargo build --features local --example extract_entry --example validate_avfx_assets --example validate_binder
python tools/verify_avfx_assets.py /path/to/game/sqpack /tmp/opencode/avfx-assets-check
```

## ATEX 对照

`tests/fixtures/atex-observations.json` 固定四条直接来自上述 Tex 块的路径及哈希。
实际条目均为 SqPack type-2；独立 Python raw-deflate 解压全部块，逐字节核对 Rust
提取结果。提取后头与 TEX 的 80 字节布局一致，mip 偏移和长度完整。
四份 2D 文件共 33 个 mip 图像、109226 像素，覆盖 BC1/BC3、128×128 与 128×256。
每级压缩数据包装为标准 DDS，Pillow 解码后逐像素比较 Rust，全部相等。
第五份 `vfx/cut/heavne/heavne11030/texture/cube004_o.atex` 来自扩展样本 #17 的
实际根 Tex 块，源 AVFX 的 DAT 位置和 SHA-256 固定在
`tests/fixtures/avfx-variant-observations.json`。该 ATEX 也是 type-2，头属性
`0x02000000`，128×128 BC1、八级 mip、每级六面。48 张图像共 131070 像素
全部与 Pillow 比较；五份 ATEX 合计 81 张 mip 图像、240296 像素。
没有把其他扩展名的资源冒称为 ATEX 的特殊类型实样本。
array/volume/浮点 ATEX 的实样本缺口保留，公共 TEX 解码证据不被改名为 ATEX 证据。

### A8、BC2、BC7 与 type-2 库存

`tools/inventory_standard_textures.py` 只读同一 Win32 安装的 VFX indexes，
对 type-2 的首块作有界 raw DEFLATE 和候选头检查。安装中匹配一个 index，
229591 条记录中均匀选择 100000 条；每包上限 100000、合计上限 400000、
首块展开累计上限 2 GiB，实际没有耗尽此预算。各次只保留一个首块，不同时
分配累计展开量。index、选中条目和首块摘要固定在外部报告，完整报告摘要和
数量固定于 `tests/fixtures/standard-texture-inventory-observations.json`。

79846 个候选头包括 2D A8/BC1/BC2/BC3/BC7/BGRA8 和 cube BC1/BC3，未见
浮点、array、volume 候选。这只约束被检查的条目和首块，不证明完整资源有效、
恢复了原始文件名或整个安装不存在其他变体。匿名 TEX-like 头不自动称为 ATEX。

`find_vfx_variants.py` 的 `pixel-formats` 模式沿 AVFX 根 Tex 块查找格式，
没有扫描任意字符串来建立引用。该有界发现确认下列三份真实 ATEX：

| AVFX index ordinal | ATEX 路径 | 编码、mip 与像素 |
| --- | --- | --- |
| 202 | `vfx/cut/aktkma/aktkma10910/texture/glow001f.atex` | A8，128×128，8 mip，21845 像素 |
| 206 | `vfx/cut/aktkml/aktkml10420/texture/clud05z_f2.atex` | BC2，128×128，8 mip，21845 像素 |
| 1142 | `vfx/monster/m0106/texture/crack13_oar13sv_bc7.atex` | BC7，256×256，9 mip，87381 像素 |

三份均为 type-2。`verify_atex_pixels.py` 校验 owner 的 index ordinal/hash、
DAT 地址、文件 SHA-256，并以独立 AVFX 分帧确认显式 Tex 路径；Rust 同时
输出全部 owner 字段、曲线和几何，与 Python 逐行相等。三份 owner 合计
103 节点、2006 曲线键、34 个内嵌模型。验证示例遍历 Str/Gra/ORad 与
Att/RdO/RdI 的结构化曲线；它们的公共曲线解析和 typed Data 类别独立存在，
没有把同名四字节 scalar 当成曲线容器。

所有 ATEX 块由 Python 有界 raw DEFLATE 与 Rust 分别提取，逐字节相等。
Pillow 的通用 DDS DXT3/BC7 解码作为压缩像素 oracle；A8 以原字节作为 alpha、
RGB 为白色，通过通用图像通道组合独立比较。全部 25 张 mip 图像、131071
像素逐位一致，包括 1×1、2×2 小 mip 的块裁边。与前述五文件合计八份 ATEX、
106 张图像、371367 像素；不据此宣称已验证 float/array/volume ATEX 或播放。
没有增加第三方实现阅读或解析依赖；Pillow 许可和使用范围见 TEX 来源记录。

`tests/fixtures/atex-pixel-observations.json` 固定三条路径、owner 身份、原始
文件及语义行 SHA-256。完整素材、owner 与像素在仓库外，联跑只使用固定样本，
不把整轮首块发现调查作为每次回归的前提。

```sh
python tools/inventory_standard_textures.py /path/to/game/sqpack /tmp/opencode/type2-texture-inventory.json
python tools/find_vfx_variants.py /path/to/game/sqpack /tmp/opencode/vfx-pixel-discovery pixel-formats
# 使用带 Pillow 的 Python 环境：
python tools/verify_atex_pixels.py /path/to/game/sqpack /tmp/opencode/atex-pixel-check
```

## 扩展变体观察与条目顺序

`tools/find_vfx_variants.py` 的有界库存：base vfx index 最多 100000 条、最多
2000 份 AVFX 和 20000 条不同纹理路径；实际检查 13159 条 index、2000 AVFX、
8814 条 AVFX 引用的 vfx 路径，其中 8809 条取得纹理头、5 条在该 base index
没有匹配记录；后者不被称为整个安装中确认缺失。选出的二十份资源覆盖 31 种带 Data 的组合，
完整资产与结果哈希固定在 `tests/fixtures/avfx-variant-observations.json`。
这批额外样本的 1328 个已知参数、2245 条静态引用和六条动画选纹曲线均逐项
与独立 Python 观察一致；ModelSkin 与 dissolve 均有真实参数块，不由源码声明
单独推定已验证。库存未找到浮点 ATEX；这只描述上述有界范围，不证明安装中不存在。

在先前已审查 MIT commit 下，再获取 LICENSE 后只查看下列类在构造方法之前的
字段声明（工具只展开 public readonly/Avfx 字段行），没有阅读这些类的解析方法：

- `Emitter/Data/AvfxEmitterData{Cone,ConeModel}.cs`；
- `Particle/Data/AvfxParticleData{Decal,DecalRing,Line,Laser,Polyline,Quad,Dissolve}.cs`；
- `Binder/Data/AvfxBinderData{Linear,Spline,Camera,LinearAdjust}.cs`；
- `Effector/Data/AvfxEffectorData{DirectionalLight,ChromaticAberration,GaussianBlur,DirectionalBlur,CameraQuake,RadialBlur}.cs`。

CameraQuake 的首次网络请求失败，重试成功后才计入上述阅读范围。
其中标签、标量类型与曲线/组类别和真实块分别对照，不复制或机械移植解析实现。
quad bMP 的一字节 ff 值保留为原 byte，不从名称直接归一化为 bool；decal 的 SS
解为 f32，与其他类型的同名整数区分。

扩展样本 #2 中偏移 6308 的 Emit 声明 PrCn=4、EmCn=1，文件先写四个累计
ItPr（各含 1..4 条记录），最后写 ItEm（含五条记录）。前四个 TgtB 为
`1,2,6,7`，最后一个为 `0`，分别属于 particle 和 emitter 的索引空间。
这直接反驳“最终累计负载总是 emitter 前缀再接 particle 后缀”的固定顺序假设。
每个条目外层标签确定其**最后追加记录**的类别；既有前缀原样保留，按 wire 顺序
输出每块自己的记录。基础六文件也据此重新逐项验证；引用的字段地址指向各自
条目块的最后记录，而非另一个块内复制的前缀。

```sh
python tools/verify_avfx_assets.py /path/to/game/sqpack /tmp/opencode/avfx-variants-check \
  tests/fixtures/avfx-variant-observations.json
```

## 复现

```sh
nix develop -c cargo build --features local --example extract_entry --example local_sqpack \
  --example validate_avfx --example validate_tex
python tools/verify_avfx.py /path/to/game/sqpack /tmp/opencode/avfx-check
# 在带 Pillow 的 Python 环境中执行：
python tools/verify_atex.py /path/to/game/sqpack /tmp/opencode/atex-check
```

输出目录必须为新目录且在安装之外。验证不运行游戏、服务器、GPU、特效模拟或
任何目标程序；提取只读取 DAT/index 字节。
