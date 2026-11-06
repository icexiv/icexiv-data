#import "../_components/pele/lib.notc" as pele;

# 天空作者输入、切片排列与采样坐标证据

## 对象与来源

本页关联三类证据：2019 年 Fan Festival 背景美术报道中的 Photoshop
画面；Win32 安装 `2026.10.06.0000.0000` 的天空 TEX base；固定客户端的
CPU 参数与 Sky shader 指令。公开画面与安装样本分别保留版本身份。

原件、反汇编、复核脚本与报告位于仓库外
`../icexiv-data-evidence/volume-production-06/`。EXE 为 51910400 字节，
SHA-256 `958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`。
native 地址采用首选 VA，image base 为 `0x140000000`；对象偏移以字节为单位，
shader 偏移相对于整个 SHCD 文件。数字字段按小端读取。

## 作者画面与 base 排列

[GAME Watch 的 2019-03-25 报道](https://game.watch.impress.co.jp/docs/news/1176185.html)
介绍志田雅人的环境制作：天空和云在 Photoshop 制作，天空按小时制作／使用
渐变，环境专用工具控制约 90 个参数。报道中的天空图为 800×452 JPEG，
SHA-256 `ab1332b6f5745886182e952300dbf881a42d9c48845a2c2a1cb1a2f0d9a9bb5b`。

| 从上至下的窗口 | 标题转录 | 可读模式文字 | 与 2026 同名 base 横排图的观察 |
| --- | --- | --- | --- |
| 1 | `sky_076.psd` | `RGB/8` | 色带形态相似 |
| 2 | `sky_023.psd` | `レイヤーマスク/8`，图层蒙版 | 色带形态相似 |
| 3 | `sky_064.psd` | `RGB/8` | 绿青色画面与同名 base 明显不同 |
| 4 | `sky_067.psd` | `RGB/8` | 色带形态相似 |

四窗的 `400%`、状态区左值 `18.0K` 可辨读，显示矩形宽高比约为 6:1。
出版图经过缩放与 JPEG 编码；这些 UI 文字和比例没有确立原 PSD 的精确尺寸。
`192×32×3=18432` 字节与该显示相容，但依赖 K 单位、大小字段含义等假设。
四窗均没有跨版本逐字节身份；第三窗差异的具体原因未知。

2026 base 的横排映射为 `atlas(8z+x,y)=volume(x,y,z)`，不变换颜色。
24 个 8×32 切片横排所得派生图确实为 192×32；这是 TEX 派生图尺寸，
没有据此读取或认定 PSD 头。四份 32 切片 PNG 回读的全部 BGRA 字节一致。

### 完整切片的重复关系

输入为 03 封存的 452 条正向天空目录捕获；清单 SHA-256 为
`7ec7eaad3536f75c3bd78d3976920ee1f01a18bfd84ccc11ebfcd2c31413a20b`。
独立复核自行从原 `.entry` 解压 base，核对条目、头与像素摘要。

| base 尺寸 | 条目数 | 核验范围 |
| --- | ---: | --- |
| 8×32×32 | 147 | 每条完整比较 z=0…7 与 z+24，全部相等 |
| 8×32×24 | 126 | 身份与完整 base 字节 |
| 8×32×2 | 179 | 身份与完整 base 字节 |

比较包含 BGRA 四通道，共 1176 对切片、1204224 字节、零差异。此处确立
有限 base 内的位移相等关系，没有证明最小周期为 24，也没有覆盖缺失低 mip。
“24 小时渐变的循环延拓”是与报道相容的解释；原始尺寸、延拓操作及 mip
生成之间的顺序仍需生产证据。

## 表参数到第三维采样坐标

图为数据流关系；`+` 后数值均为对象／参数块内的字节偏移。

#pele.diagram(r#"""
flowchart TB
    table[Sky table: u16 at record +8] --> n[Renderer +0x44bc / +0x44be: N]
    input[Input u divided by 86400] --> t[Renderer +0x44b8: t]
    tex[Texture +0x50: D] --> calc[Separate multiply, divide and add]
    n --> calc
    t --> calc
    calc --> cb[cSkyParam +0x40 / +0x44]
    cb --> sample[Sky / Sky2: third sample coordinate]
"""#)

### CPU 字段与条件

天空表位于 `0x142119070`，600 项、stride 16。`1400801be/1400801d0`
读取表项 `+8` 的 u16，经 `1400801c2/1400801d4` 写到 renderer
`+0x44bc/+0x44be`。两个 ID 来自 context `+0x08/+0x20`；超出 600 的
unsigned ID 使用第零项地址。272 条短天空资源对应的 N 均为 24；
`sky_400` 没有对应表项。

texture 从 context `+0x10/+0x28` 转交 renderer `+0x42b8/+0x42c0`。
第一 texture 被过滤而第二有效时，`140080163..140080173` 将第二 texture
及权重移入第一槽，两个表 ID 没有交换。因此 N 与 D 的关联应按 renderer
槽及分支解释，不能无条件配回原始两张纹理。

effect 的 `R15` 来自 global `0x1428fcf10` 所指对象。纹理非空时，
`14036071e/14036072c` 从其 `+0x50` 取 D；空指针分支采用 1。
`140306550..1403065c2 → 14022487b` 确认正常 TEX 创建路径的该字段来自
`max(header_depth >> selected_mip,1)`，不必等于原 TEX base 深度。

`1400754ba → 140075944 → 140075990 → 1400770a1` 将入口 float32
输入 u 除以 `86400.0f`，再写入 renderer `+0x44b8`。常数字节为
`00 c0 a8 47`。两个确认的直接调用点从另一对象的 `+0x10` 重读 u；
其中一条循环按 `i×86400/K` 构造输入，并有可选字段覆盖。
这支持日内归一化相位的解释；钟源、真实秒／游戏秒、全部写入者和运行时
取值范围没有由这条局部链确定。

逐相位循环的[CPU 投影与缓存证据](volume-sky-consumer.md)另外关联了对象
mip 0、逐方向采样及九基累加。记录中的相位值与可覆盖的环境输入分开解释。

令 N、D 为转换后的 float32 值。CPU 的运算顺序为：

```text
t = divss(u, 86400.0f)
h = divss(0.5f, D)
p = mulss(N, t)
q = divss(p, D)
w = addss(q, h)
```

忽略各步舍入时，`w=(N×t+0.5)/D`。原指令分别执行 SSE 乘、除、加，
没有融合成一次舍入；MXCSR 运行时状态未知。结果位于 `RBP+0x150/+0x154`，
`140360b79` 的参数块从 `RBP+0x110` 开始、长 `0x60`，名称为 `cSkyParam`。

### Sky shader 的直接消费

两条路径均来自 EXE 的完整字面量：`shader/sm5/posteffect/Sky.shcd` 与
`Sky2.shcd`。对固定 index 中这两条 type-2 记录进行有界重取与独立解压，
完整文件摘要与[单 mip 证据](volume-mips.md)的既有身份相同。

两者为 PS 5.0；RDEF 将 `cSkyParam` 绑定到 slot 2，大小 96 字节，变量为
六个 float4。token 声明确认对应 texture 是 3D。

| 文件 | MOV 偏移 | 紧邻 SAMPLE 偏移 | 第三维来源 |
| --- | --- | --- | --- |
| `Sky.shcd` | `0x64c` | `0x664`，t0/s0 | `cb2[4].x`，参数块 `+0x40` |
| `Sky2.shcd` | `0x6ec` | `0x704`，t1/s1 | `cb2[4].y`，参数块 `+0x44` |
| `Sky2.shcd` | `0x750` | `0x768`，t0/s0 | `cb2[4].x`，参数块 `+0x40` |

MOV 后直接 SAMPLE，没有第三维的附加算术变换。三处为普通隐式 LOD
采样，LOD 限制仍由[已登记 sampler 链](volume-mips.md#lod-限制)确定。
这条链解释 base 切片的消费；它没有计算或写出缺失 mip。

在忽略舍入的代数模型中，`D×w−0.5=N×t`。若 N=24 且输入满足
`0≤t<1`，base 的连续切片坐标覆盖 `[0,24)`，与 D=24 或 32 分开。
depth32 的第 24 片等于第 0 片，与末端插值接回首片的解释相容；这项
条件性计算不证明所有运行时 t 的范围，也不决定其余 mip 的内容。

## 生产工具资料的范围

- CEDEC 2018 本多圭的《グラフィックスアセットビルド環境》讲演简介描述
  旧资产转换、新资产量产及依赖／版本管理。CEDEC 页面与 CEDiL 1838 的身份
  一致；资料下载要求登录，没有取得讲演正文。Square Enix 技术目录未给出
  该讲演的链接；xiv.zone 对应项只列 TODO。
- CEDEC 2014 青野晃的 SqPack 讲演及 Famitsu 报道提供 Perforce/Jenkins
  构建关系、`data.ps3.tex` 等平台中间名、PS3 的 16 KiB 块及 SPU→VRAM
  流程。该平台讲演没有给出 Win32 天空 mip 生成或前缀 writer 参数。

## 来源、许可与验证身份

公开讲演简介、报道和图片按各发布者版权保留在外部证据目录，只作来源
参考；xiv.zone 页面标示 CC BY-SA 4.0。Defuddle 0.19.6 为 MIT，Pillow
12.3.0 仅用于图像重排与最近邻放大。两份图片页的 canonical URL 提取警告、
Famitsu 首次正文提取失败及其嵌入 JSON 第二 body 的成功提取均保留。

DXBC token 使用 Microsoft DirectXShaderCompiler commit
`ea3489109cc8a968b36e36ca20a698a68b516d32` 的声明头。先审根 LICENSE.TXT，
再读取 `d3d12TokenizedProgramFormat.hpp`；University of Illinois/NCSA
声明及 Microsoft 版权保留。两者摘要分别为
`27a49e35d1da96eba18fba54bc882667ff0ff8c0254f16f2b6e165d605ba7df8`、
`cf808b363406fce9387d7ec8daec19940175af966589c0d3e084a4c41c3f5060`。
没有编译 DXC 或引入实现依赖。

截图／base 的独立复核使用自写 type-4 解包器；native 复核独立映射 PE
地址，核对 20 份 raw、102 个指令锚点及 chained unwind；shader 坐标按
原始 token 与声明核对。工具为 Python、zlib、GNU objdump 2.46，均为
离线观察，不是 REA provider Evidence 或运行期采样。

固定报告摘要见 `tests/fixtures/volume-production-observations.json`。
`SOURCES.md` 记录自动展示但未经预审的第三方实现搜索片段；未主动打开或
采用这些实现，既有接触与许可证声明完整保留，不宣称严格 clean-room。
六级完整重建为 **0/273**，缺失 **603624** 字节。生产转换器、滤波精度、
抖动相位及前缀写出过程分别需要可独立核对的证据。
