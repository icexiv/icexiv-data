#import "../_components/pele/lib.notc" as pele;

# Volume 生成候选的 CPU 数值证据

## 对象与执行环境

对象为 Win32 `2026.10.06.0000.0000` 的 `sky_441/067/104/358` 四条固定
捕获，身份沿用 [Volume 缺口证据](volume-generation.md)。仓库外目录
`../icexiv-data-evidence/volume-generator-02/` 保存原源码、许可证、构建
记录、CPU 输出及报告。输入仅来自持久捕获，4 个 entry 与 24 个组的摘要
逐一核对；实验没有读取安装或运行客户端。

CPU 对照使用 Linux x86-64、GCC 15.3.0、SSE2、nearest-even，编译参数
含 `-O2 -ffp-contract=off -fno-fast-math`。上游源码未经修改；适配器调用
原始 API、限制输入尺寸，并只导出实际已存坐标上的候选前缀。源码控制流、
CPU 输出和有理数推导分别给出，不能将其等同于 FFXIV 生产端身份。

`../icexiv-data-evidence/volume-generator-03/` 保存直接 base 张量候选的
452 条资源对照、归一化/乘加分支、源域反例及独立核对。该目录另有授权
安装的正向路径捕获；数值计算只读取持久捕获，详见下述目录总体。

## DirectXTex 的具体路径

固定版本的 `CreateTriangleFilter` 构造源像素对目标像素的贡献表。14 个
轴长组合与样本提出的周期线性单元积分系数比较，除 24→1 外逐项精确相等；
24→1 的 binary32 系数最大误差为 `41/402653184`，总权重误差为
`21/134217728`。

`Generate3DMipsTriangleFilter` 读取 `GetImage(level-1,0,z)`，每级减半
三轴尺寸；按源 z/y/x 递增累加，权重相乘顺序为 z×y×x。源码注释中的
“Resize base image to each target mip level”与实际读取索引须区分。
BGRA8 写出经 `StoreScanlineLinear → StoreScanline`，没有抖动调用。

以下是 CPU 调用关系图；节点为处理阶段，没有字节或 bit 坐标：

#pele.diagram(r#"""
flowchart TB
    base[Captured BGRA8 base] --> direct[GenerateMipMaps3D: recursive TRIANGLE]
    direct --> ordinary[BGRA8 ordinary store]
    base --> convert[Convert to RGBA32F]
    convert --> floats[GenerateMipMaps3D: float accumulation]
    floats --> quantize[Convert to BGRA8 with ordered dither]
    quantize --> compare[Compare captured prefixes]
    ordinary --> compare
"""#)

四条捕获的五个低 mip 共 4528 字节，三个明确调用组合的结果为：

| 调用组合（三轴 WRAP） | 不同字节 | 最大绝对差 |
| --- | ---: | ---: |
| BGRA8 → TRIANGLE mip | 758 | 16 |
| BGRA8 → TRIANGLE mip，加 DITHER flag | 758 | 16 |
| base 转 RGBA32F → TRIANGLE mip → BGRA8 DITHER | 725 | 17 |

前两行的全部比较字节相同。第三行能复现 `sky_441` 的两个指定点，完整链
仍有反例。这排除了上述固定调用组合直接解释四条已存链的可能。

## 两个反例的可执行机制

### 周期边界与累加顺序

`sky_441` mip 1 的 `(x,y,z)=(1,0,1)` 与 `(1,4,1)` 具有相同完整邻域、
相同有理滤波值 `6373/32` 和相同相位。局部 y 索引按 `[0,1,2,3]` 编号，
第一点的局部首行回绕到源 y=31。按源 y 递增访问时，两点的局部次序分别
为 `[1,2,3,0]` 与 `[0,1,2,3]`。

原始浮点 mip 输出的三色通道 bit pattern 分别为 `0x3f47eff0` 和
`0x3f47efef`，相差 1 ULP。原始 `Convert(DITHER)` 分别输出
`[200,200,200,255]` 与 `[199,199,199,255]`，与实际 BGRA 相同。
同邻域有理数相等不要求这个源顺序累计过程产生相同 binary32 结果。

### 从 base 直接累计的非 tie 越界

全体均值探针将 `sky_067` base 8×32×32 的同一字节顺序解释为 8×1024，
调用原始 `Resize(TRIANGLE|WRAP)` 直接到 1×1，再执行原始 BGRA8 DITHER。
原始系数逐项确认 8→1、32→1、1024→1 均为精确二次幂均匀权重。
`y_flat=z*32+y` 保持源 z/y/x 次序，逐像素乘积权重为 `1/8192`；在这个
全体均值条件下，与直接三维累计的贡献及次序相同。该等价关系不推广到
中间 mip 或任意深度。

精确 B 均值 `204927/4096` 比阈值 `50+1/32` 小 `1/4096`。CPU 输出
归一化 B 的 bit pattern 为 `0x3e48eadb`；精确乘 255 得
`3357667365/67108864`，约 50.03314264，比阈值大 `127013/67108864`。
binary32 缩放并加抖动偏置后的舍入输入为 50.50189208984375，得到 B=51。
完整输出 `[51,35,37,255]` 与实际末级相等。

同一均值探针对完整对照 `sky_104/358` 的末级分别输出 `[13,3,8,255]`
和 `[78,51,21,255]`，亦与实际相等。三个末级点不能代替完整六级生成验收。

## 奇数行相位的显式适配

原始 `StoreScanlineDither` 按物理 x 选择相位，使用
`j=(z+x) mod 4`。奇数行遍历反转不改变这个索引。以归一化常量 0.5、宽
1/2/4 和四种 y/z 相位核对的 112 个 B 通道全部符合该公式，其中 48 个与
[样本的反向相位](volume-generation.md#bayer-量化关系及精确反例)不同。

相位探针在原始抖动函数前后反射奇数行，y/z 保持不变，复用预先固定的
样本相位；这是适配器操作，DirectXTex 没有对应参数。其四条捕获 mip 1
的 **4096 字节全部相等**，包括两个完整非退化对照的 mip 1。
mip 1 的源就是 base，此项不验证逐级递归或每级直接从 base 的差别。

两个反例分别获得了可复现的浮点机制；完整统一生成器、生产端的相位来源
及 `D>>m` 到 `D>>(2*m)` 的长度/box 写出链仍需来源证据。273 条短资源
的六级完整重建数为 0，缺失逻辑字节 603624。

## 直接 base 张量的目录总体

### 目录与样本身份

固定 executable 的天空表 `0x142119070` 含 600 项；路径指针及字符串经
PE 映射实读，目录为 `bgcommon/nature/sky/texture`。加上独立定位的
`sky_400.tex` 后为 595 条已知路径；唯一选中 `010000.win32.index` 主表
含 77434 记录，593 条路径唯一命中，`sky_123/392` 在该主表无命中。

593 个已定位头中，478 为 volume：452 个六级 8×32×2/24/32 条目、25 个
单 mip 8×32×24 条目及一个六级 4×32×2 条目；另 115 个为非 volume。
实验范围选定为其中 273 个短 volume 与 179 个完整 depth=2 对照，分别
去重为 177/176 条链。272 个短条目来自 native 表，`sky_400` 单列。
此范围是正向天空目录；不等同于全安装库存。

捕获读取 DAT 1538088 字节，解压 8871472 字节，文件前后身份相同。全部
452 个 entry、2712 个组和四个固定见证摘要均保留。独立审核确认两种头
形状：普通累计 mip offsets，以及六个 offsets 均为 80 的短 `sky_267`
和完整 `sky_380/382/383/387`；packed 头没有消除短条目的长度缺口。

### 预声明数值契约

每个低 mip 直接读取原 base，轴权重分别使用周期线性单元积分或已导出的
DirectXTex 系数；各轴先舍入到 binary32，再按 z×y×x 相乘。贡献按原始
源字节地址递增累计，归一化和末端抖动调用 02 的未修改原件，奇数行相位
使用上述显式反射适配。没有拟合 epsilon、偏置或逐资源例外。

以下是数学候选的阶段关系图，节点没有字节坐标；目标只包含实际已存位置：

#pele.diagram(r#"""
flowchart TB
    base[Captured base] --> normalize[Original UNORM8 normalization]
    normalize --> tensor[Direct base tensor for each mip]
    tensor --> separate[Separate binary32 multiply and add]
    tensor --> fused[Original FMA multiply-add branch]
    separate --> phase[Fixed phase and original dither]
    fused --> phase
    phase --> compare[Compare stored target coordinates]
"""#)

| 比较域 | 每种候选的字节数 | separate 差异 | FMA 差异 |
| --- | ---: | ---: | ---: |
| 四个固定见证的 20 个低 mip | 4528 | 0 | 0 |
| `sky_372/111/387` 的 15 个完整低 mip | 1044 | 0 | 0 |
| 452 条资源的 2260 个低 mip | 591408 | 131 | 113 |
| 其中 273 条短资源 | 529116 | 89 | 79 |
| 总体按 353 条链去重 | 431196 | 128 | 111 |

最大绝对差均为 1。rational/exported 两种权重的 separate 量化输出相同；
FMA 使用 rational 权重。编译的原始 SSE 乘加与 Python separate 归约的
431196 个 float 标量逐位相同。FMA 的宿主 CPU 特性已检查；这些构建
参数没有被归属给 FFXIV 生产端。

## 归一化与剩余反例

原始 DirectXMath `XMLoadUByteN4` 的 SSE/scalar 两分支在 256 个输入中
有 126 个 1 ULP 差异，scalar 值逐输入不大于 SSE。`sky_003` mip 1
`(2,12,3)` G 的 separate 候选为 46，实际为 47。正权重、固定顺序舍入
及同相位量化具有单调性，因此仅换 scalar 归一化不能修正该低估；小见证
通过后没有执行其目录总体计算。

separate 的 128 个去重差异全部位于精确积分阈值，其中 104 个也是
binary32 量化 half-tie。FMA 修正其中 41 个、引入 24 个差异，双方共有
87 个；按短条目展开为修正 28、引入 18、共有 61。FMA 的 111 个残差均在
精确积分阈值，109 个为 binary32 half-tie。

诊断性地保留精确乘 255 和加偏置，分别解释 separate/FMA 残差中的 83/99
个。该计数只针对残差集合，不是更高精度候选的全域通过数。

### 相同候选输入的不同实际输出

`sky_005` mip 1 的 `(0,3,5)` 与 `(1,3,6)` B，在 separate 中间结果中
都为 `0x3f259d9e`，Bayer 相位均为 `(y mod 4,j)=(3,0)`，实际却为
164/165。仅依赖该 float、通道和相位的确定性逐点量化函数不能同时解释
两点；因此单换这种末端量化函数无法修复该候选。隐藏空间状态或其他
滤波中间值属于不同假设。

### FMA 的非 tie 反例

| 见证 | binary32 量化输入 | 候选 | 实际 |
| --- | --- | ---: | ---: |
| `sky_259` mip 1 `(1,7,4)` G | `16711681/131072` | 128 | 127 |
| `sky_277` mip 1 `(1,10,4)` B | `10452991/65536` | 159 | 160 |

两个输入均不在 half-tie，单换 tie 规则不能消除反例。

## 独立核对

03 的独立块表/DEFLATE 观察器核对全部 452 个 entry、2712 个组、目录映射
与比较计数；短条目的 1338 个 DEFLATE 和 720 个 raw 块均精确耗尽。
353 个 base 的 5873664 个归一化 float 标量与归一化表一致。头分类复核
只重读固定 executable 与 141 个排除头的 14664 DAT 字节，文件身份一致。

数值复核从三角基函数原函数积分构造权重，用整数商余数实现 binary32
nearest-even。8 个指定点的 separate/FMA 中间位和最终字节逐项相等，
包括两个旧反例、等输入冲突和 FMA 非 tie；另复现
[7 个源域反例](volume-generation.md#缩短源域的条件性反例)。该复核没有调用
候选计算器或外部库，也不依赖宿主浮点舍入。

初始链接缺失对象、目录捕获在 4×32×2 处的尺寸断言及独立审核对全 80
头的初始假设均登记于外部 `REPORT.md`。162 个已写 entry 及其组由重新
实读逐字节核验后复用；没有丢弃失败过程中的产物。

## 历史版本与源贡献序列

`../icexiv-data-evidence/volume-generator-04/` 固定审阅官方 DirectXTex
jan2014、feb2014、mar2015 的九个原件。三版 3D TRIANGLE 均读取
`GetImage(level-1,0,z)`，BGRA8 经 linear store 写出，没有有序抖动调用。
独立逐行审核确认宏、ordered 分支、源级选择和归约共 18 个锚点。

February 2014 抖动修复的具体语义是：偏置已经加到 `target`，舍入的
操作数由 `v` 变为 `target`。三版均按物理 `index & 3` 选择偏置；奇数行
逆序遍历没有反射相位。因此该修复没有提供样本奇数行相位的来源。

04 对 152 个残差种子另作输入同一性检查。比较域为 353 条去重链的实际
已存 mip 1/2/3，尺寸、mip、通道和相位须与种子一致；共选中 223366 个
坐标通道。签名保留源 z/y/x 顺序、各轴精确系数与源 byte，分别比较单通道
和全部 BGRA 两种序列。每种得到 **26 个有向匹配，即 13 对不同坐标**，
实际输出全部相同。

独立复核用折叠整数 tap 构造系数，并逐字节比较完整命中序列，计数一致。
这没有取得更广泛的确定性冲突，也没有确定某个生成规则；签名不覆盖其他
源位置、零权重访问或隐藏空间状态。

## Scalar 与 x87 的实际精度

04 以 02 的未修改 MIT 原件构建 scalar 分支，使用六个固定 base 和八个
既有见证所在行。每份构建比较 29 个已存像素、116 个 byte；门槛出现反例
即停止扩大。没有拟合偏置、epsilon 或逐资源例外。

GCC 15.3.0 的 `-mfpmath=387 -D_XM_NO_INTRINSICS_` 不能单独证明扩展精度：
允许自动向量化的构建实际用 `divps` 归一化、`mulps/addps` 归约。
禁用 `tree-vectorize/tree-slp-vectorize` 的构建则使用 x87 除法，并在归约
循环中将四个累计量保留于 64 有效位寄存器，循环后才写入 binary32。
FPU control word 为 `0x37f`；两种量化路径均在乘 255 后和加偏置后存为
binary32，再执行 nearest-even 整数舍入。

以下为自建 CPU 对照的精度关系图，节点没有字节坐标：

#pele.diagram(r#"""
flowchart TB
    base[Captured BGRA8] --> norm[Divide by 255; store binary32]
    norm --> sse[SSE binary32 multiply and add]
    norm --> x87[x87 register accumulation: 64 significant bits]
    sse --> store[Store binary32 row]
    x87 --> store
    store --> quant[Scale and bias with binary32 stores; nearest-even]
    quant --> compare[Compare the same eight observed rows]
"""#)

| 实际编译路径 | 比较 byte | 不同 byte | 不同见证 |
| --- | ---: | ---: | ---: |
| 自动向量化的 SSE/scalar 混合路径 | 116 | 2 | 2 / 8 |
| 禁用自动向量化的 x87 路径 | 116 | 7 | 5 / 8 |

两者都把 `sky_003` mip 1 `(2,12,3)` G 算为 46，实际为 47；都将
`sky_005` 的两个指定 B 点算为 164，实际为 164/165。x87 路径还把
`sky_441` 的 200/199 算为 200/200，把 `sky_067` 末级的 B=51 算为 50。
这些反例不支持用该扩展精度契约重建缺口；两份候选均没有执行目录总体。

独立整数有理数复核从三角基函数积分构造权重，实现 24/64 有效位
nearest-even，重放反汇编确认的存储点。每份构建的 163840 个归一化标量、
116 个中间 float 位及 116 个最终 byte 全部一致。该结果核验 CPU 实验，
生产编译器、调用身份与前缀写出链仍未建立，完整重建数为 0/273。

## 来源、许可与阅读范围

采用源码均先审根许可证，保留原件和版权通知；第三方源码及构建产物位于
仓库外，项目没有引入对应依赖。

| 来源 | 固定 revision | 许可证 |
| --- | --- | --- |
| Microsoft DirectXTex | `c0cb97764379ebd20dbd2016c58965db85f5b307` | MIT |
| Microsoft DirectXMath | `ee612d379528a69d11df7dfb72f5e656e909376a` | MIT |
| Microsoft DirectX-Headers | `75edcf75f4c0942dd0d39e75fdc5c42ca6f285fd` | MIT |
| dotnet/runtime SAL | `07b1f05c43023f6352f5479306f9a9673bf2f715` | MIT |

DirectXTex/DirectXMath/SAL 的许可证身份沿用 [BC6H 来源](bc6h.md)。
DirectX-Headers 根 MIT LICENSE 的 SHA-256 为
`903df5512f7d02609fed0c780a9b704f5a3eeb6e4d84ebe42a29845c81899a3c`。
前三项版权人为 Microsoft Corporation，SAL 为 .NET Foundation and Contributors。

定向阅读 DirectXTex 的 Mipmaps、Convert、Resize、filters 相关函数及
平台/公共声明；DirectXMath 的 UNORM8 载入与 multiply-add。编译原始
Mipmaps、Convert、Image、Misc、Util、Resize 六个翻译单元，辅助单元正文
不作为逐项算法审阅结论。`.d` 文件与来源清单固定编译器消费的文件。

[官方 NuGet 包](https://www.nuget.org/packages/Microsoft.DXSDK.D3DX/9.29.952.8)
说明对应 June 2010 SDK；其[许可证](https://www.nuget.org/packages/Microsoft.DXSDK.D3DX/9.29.952.8/License)
§1(a) 限定 Windows 使用，§4(b) 限制反编译/反汇编并保留适用法律及第三方
条款例外。此来源未被采用；仅归档包页及许可证，没有下载 nupkg/DLL 或执行、
反汇编该 SDK。

网络搜索自动展示的第三方 DirectXTex 镜像/提交片段另登记于证据目录的
`SOURCES.md`；这些结果没有逐项许可审查，未用作实验实现或生产来源。
既有实现接触记录保留，不宣称严格 clean-room。

03 复用上述原始对象及头，定向读取归一化、multiply-add 与有序抖动分支；
原件身份沿用 02 封存。作者工具的公开搜索只取得访谈和其他作品讲演摘要，
未作为 FFXIV 规则或实现依据；查询及附带接触登记于 03 的 `SOURCES.md`。

04 历史源码采用的 revision 及许可如下；实现正文读取前审阅许可，原件、
通知和完整接触记录保留在仓库外 `SOURCES.md`、`license-review.json`。

| 官方 DirectXTex 标签 | revision | 许可与使用范围 |
| --- | --- | --- |
| jan2014 | `73267aa79794220f9c283aac8955ab06627054ef` | MS-PL，只审阅 |
| feb2014 | `183937c3f767c19b41d05152ca53dea1757bceaa` | MS-PL，只审阅 |
| mar2015 | `b036f8906b3ff96deed0526c114ca1596f4d09be` | MIT，只审阅 |

两个 MS-PL 声明均在原 ReadMe.txt 74–75 行；官方许可全文归档 SHA-256
为 `199ddd959e789d316454cb75a52560e5fdd70ea9a925441d3893aca718186ccc`。
mar2015 的 MIT.txt SHA-256 为
`827b004cab2693190e41b0d03f6aa66ebac1a60befa8cb222410fe2db68e5faa`。
历史原件没有编译或移植，CPU 构建使用 02 的 MIT 原件。源文本含非 UTF-8
字节，行锚点采用 Latin-1 字节映射；失败检索和宏起点纠正记录亦保留。

## 固定报告身份

| 产物 | SHA-256 |
| --- | --- |
| `build-sources.json` | `5ebcba94f608e142aaaf0f1e031f6edfba5cd32b9fb69c0b47cfeb609df1fc89` |
| `comparison.json` | `6ebeb167ba2436be533b7692f6d9990d5987f3ebf8ba990626ff8bb8196d5463` |
| `mean-probe.json` | `ae6f62a49071c1278db302bcca37bc9133e8d52188829ee23eb6a585df0f184d` |
| `phase-probe.json` | `029524545c7a9d9040a0cb3c38ffd88474ab18dfbcfded0b5684920329b60e71` |
| `verification.json` | `ce5231accc7ac97bb72bf45b557376cf070ebe67c3bf7f032622b436ac60afcd` |

来源清单、构建命令、原件、完整报告和 SHA256SUMS 均在持久证据目录。
仓库观察身份固定于 `tests/fixtures/volume-generator-oracle-observations.json`。

03 封存 18080 个文件、49766187 字节；清单 SHA-256 为
`84a6d6a0bc642240b594da45252ce1722c9039cef174822e76558de592ab2020`。

| 03 产物 | SHA-256 |
| --- | --- |
| `cohort-catalog.json` | `7ec7eaad3536f75c3bd78d3976920ee1f01a18bfd84ccc11ebfcd2c31413a20b` |
| `comparison-catalog.json` | `cb163e914bfe2c9532639f79151089258594a3eae86a72b406060c3c8a013866` |
| `fused-catalog.json` | `23e796dcda41b3f9f24a61b815badca18afc4db339c8c680ebb2ca3867d4eb9b` |
| `reduction-residuals.json` | `f99017b3c62e1b4bb313b2eac0d5b2919bab58f618b0d22a0e941b1732b3f3f8` |
| `arithmetic-verification.json` | `bf9948855285519baff81209c108a73c96031781da7e9c11163f7e1ab4265f5d` |
| `archive-validation.json` | `2271c8a2539a411fb77bcb1a8906deae717e6bedf54a21669dfdf252324d8d57` |

完整身份与范围固定于 `tests/fixtures/volume-generator-cohort-observations.json`。
封存复核入口为仓库外 `volume-generator-03/seal_bundle.py`；已有清单时只校验。

04 封存 227 个文件、4966191 字节，清单 SHA-256 为
`0c5a313f0e53e33a6c1147b7ff1ed2074b4a0d7b495fd2581ed56c1814230595`。

| 04 产物 | SHA-256 |
| --- | --- |
| `source-history.json` | `0ad70a1878c500c29d5891b1ef922738e5e970101153d8810b3b19ecfabc2d1d` |
| `source-traces.json` | `fecd59202eb836b68885376c6bcf2274e1558e6e0a485a21ad8e61a91d273c52` |
| `trace-verification.json` | `b0194c602d0ad12afc0c433993a8ce1f084da833a2abcefc2b349e272920ceff` |
| `x87-gate.json` | `ef534dbfc27221875e0337631bd2aa5edfefce319c8afc278a701492fc0566bf` |
| `x87-no-vectorization/x87-gate.json` | `9e0962fb08fd0bfa065f5b735dcc77cfca6d8a75a1ee8a68d86c31c5308a8970` |
| `x87-arithmetic-verification.json` | `c9bcfd68b94d575ea4f0f4521a000418e4fe21966ba5d971143ec06529e6e11f` |
| `archive-validation.json` | `514d7c091d37a26d0127d9a0506ba7909bf23544b445c1fca18f2d524385bec8` |

固定观察见 `tests/fixtures/volume-generator-precision-observations.json`。
04 的 `seal_bundle.py` 在清单存在时只校验，没有重复执行候选或修改封存。
