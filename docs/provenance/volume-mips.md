#import "../_components/pele/lib.notc" as pele;

# Volume 单 mip 像素与天空采样证据

## 对象与来源身份

对象为 Win32 安装 `2026.10.06.0000.0000` 的 273 条 type-4 短 volume
及一条完整对照。读取规则按独立数据组提供完整 mip 的全部切片，保留原始
六级 TEX 头。273 条的 base mip 均有完整真实像素；六级完整重建数为零。

固定 `ffxiv_dx11.exe` 长 51910400 字节，SHA-256 为
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`，
首选 image base 为 `0x140000000`。以下 native 地址均为首选 VA，对象字段
偏移单位为字节。指令观察使用 GNU objdump 2.46；REA 原生引擎不可用的
边界沿用来源总表，没有执行客户端、附加进程、shader/GPU 或服务器。

外部研究目录 `/tmp/opencode/volume-resolution-01/` 保存捕获、原始像素、
反汇编、工具和许可证；库输出复验保存在
`/tmp/opencode/volume-mip-verification-01/`。这些游戏资源字节不入仓库。
固定身份登记在 `tests/fixtures/volume-mip-observations.json`：

| 产物 | SHA-256 |
| --- | --- |
| `selected-manifest.json`：274 条捕获位置、头部、组与完整 mip | `cc4beb7cdb5129ad49baec01eea107ab032be23fcad356563a1fb32684c828b3` |
| `path-identities.json`：路径、双 index 与 native 天空表 | `2c193f4f204e4514c708f128a5b49c4e4bd8aef1520219ab7e974401847d5170` |
| `sampler-evidence.json`：固定指令锚点与 sampler 推导 | `23f596c160d9c8543aba824603e08985a6947ac42fc32da72ba35380b91d06e0` |

478 个既有 bgcommon volume 路径经 index/index2 双查询指向同一 DAT 位置。
273 条短资源含 178 个压缩条目摘要、177 个 base 像素摘要；按资源条目计数，
不将内容重复项称作不同像素变体。没有为这项验证重复执行全索引负向扫描。

## 组与完整 mip 的正向规则

一组一 mip 的 volume 将 type-4 组 ordinal 与原始 mip ordinal 关联。
每组以自身的物理偏移、首块索引和 u16 跨度解包；TEX 尺寸计算完整负载。
BGRA8 的长度为 `4 × max(1,W>>m) × max(1,H>>m) × max(1,D>>m)`。
全部有效 offset 为 80 的完整对照按连续尺寸跨度定位，原始 offset 字保持。

| 路径末尾 | DAT 与字节位置 | 基础尺寸 | 组实际长度 | 完整 mip |
| --- | --- | --- | --- | --- |
| `sky_441.tex` | `ffxiv/010000.win32.dat1@967239424` | 8×32×24 | 24576,1536,64,16,8,4 | 0,4,5 |
| `sky_045.tex` | `ffxiv/010000.win32.dat0@1609282304` | 8×32×32 | 32768,2048,128,16,8,4 | 0,5 |
| `sky_387.tex` | `ffxiv/010000.win32.dat1@967331456` | 8×32×2 | 2048,256,64,16,8,4 | 0–5 |

三条路径前缀均为 `bgcommon/nature/sky/texture/`。前两条声明的完整长度为
28172、37540，差额为 1888、2488；第三条为 2476 字节，六个 offset 均为 80。
短资源中另有一条重复-80 头，头部形状本身不证明完整负载存在。

验证范围包括所有组表、跨度、物理范围及有序不重叠关系；所选组的每个块
另检查存储长度、DEFLATE 流结束、输入恰好耗尽与输出精确长度。所选组必须
覆盖该 mip 的全部切片，才形成完整像素输出。原始头的完整逻辑长度、mip
总数和组数量也独立核对。失败的其他流不影响已验证组的字节含义，但单组
读取不会据此承诺整个条目的压缩流有效。

## 公共库输出的独立对照

`examples/validate_volume_mips.rs` 通过内存 `PackSource` 重放原始捕获，调用
公共 SqPack 单组读取与 TEX 完整 mip 解码。`tools/verify_volume_mips.py`
从同一固定捕获独立解析组/块，用 Python raw zlib 解压并交换 BGRA 的 B/R
通道；Rust 复用 `flate2` 的 miniz 后端。两个执行后端共享样本身份和字段
事实，独立性指解包与逐字节比较，像素不是客户端运行截图。

每个成功项比较完整 encoded payload、RGBA、尺寸、原始 mip 编号/总数；
每个失败项核对精确 `MipSizeMismatch` 和不存在像素输出。原始 80 字节头
逐字节比较；全条目读取另行检查。

| 验收项 | 数量 |
| --- | ---: |
| 短资源的完整 base mip | 273 |
| 这些 base mip 的切片 / 像素 | 7728 / 1978368 |
| 短资源中完整的单 mip | 672 |
| 完整对照中的 mip | 6 |
| RGBA 比较字节 | 7917968 |
| 不同字节 | 0 |
| 明确拒绝的不完整 mip | 966 |
| 完整文件读取拒绝的短资源 | 273 |
| 短资源六级完整重建 | 0 |

固定报告 SHA-256 为
`b5667a7c86197d654fbdfe3631e4510c3bc853090d5ff21cab4e21372a885126`。
按 manifest 顺序拼接 `name + NUL + u8 mip + RGBA` 的身份摘要为
`89c184907edef6d7cc6532f7e5b0c0f28ab79650ac8220f8230ce5dd09d53ba5`。
`execution.json` 记录 Python、验证脚本与实际 Rust 验证器的执行身份。

```sh
nix develop -c cargo build --example validate_volume_mips --locked
python -B tools/verify_volume_mips.py /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/volume-resolution-01 /tmp/opencode/volume-mip-check
```

最后参数必须是不存在的外部目录。重放只读取固定捕获及 executable 锚点，
不需要重新扫描安装。合成回归覆盖组预算、全表范围、IO 错误、未请求流
损坏、逻辑身份和短/长 mip；TEX 的数组浮点与 BC cube 测试也比较单 mip
绑定和完整文件读取的所有图像。API 规则与用例见库 README。

## 天空加载与 sampler 的静态关联

### Texture 来源

native 天空表位于 `142119070`，600 项、stride 16，路径指针在 `+0`，
u16 参数在 `+8`。其中包含 272 条短资源；这些条目的 u16 值均为 24，参与
[第三维采样坐标的相位缩放](volume-production.md#表参数到第三维采样坐标)。
`sky_400.tex` 有路径/字节/像素验证，没有对应的天空表项。

- `14007f480` 选择表项，`14007f557` 经 `1402feae0` 请求资源并存入
  context `+0x60`；请求记录中的 `+8=3` 没有被解释为 mip 限制。
- `14007fafc..14007fb04` 将 context `+0x40` 与 `&context[+0x60]`
  交给 `140087df0`；`140087e5a` 从 handle `+0x128` 取 texture，写入
  目标记录 `+8`。切换后两个 texture 字段位于 context `+0x10/+0x28`。
- `1400800f2/14008010a` 读出两个 texture，`140080202/140080229`
  写入 renderer `+0x42b8/+0x42c0`，天空 effect 再读取这些字段。
- volume 创建分支 `1403064d0 → 1402247b0` 在 `140224886` 将 texture
  `+0x55/+0x56` 清零；调用者回写 `+0x55`，该处没有改写 `+0x56`。
  创建属性清除 `0x800000`，不满足天空特殊转换分支的初始条件。

### LOD 限制

`Sky.shcd` / `Sky2.shcd` 反射中的 `sSky1/sSky2` 为 3D 资源；普通 `sample`
指令使用隐式 LOD。固定 LOD 来自 sampler 描述，不能从 shader 的采样指令
自身推出 mip 0。两份 shader SHA-256 分别为
`3a6150c72bb530c28255b1632e81c5767801cd9eb939e633e2e41d8dbe14cbda`、
`74a0fe4ab0411620a6c2fece0935f24cbeb27c45a775e9afd0757a3369617b02`。

以下为调用关系图，节点中的 `+` 偏移单位为对象内字节：

#pele.diagram(r#"""
flowchart TB
    sky[Sky binding: sampler flags 0x24a] --> effect[Effect to thread record +0x1150]
    effect --> packet[Command record +0x10; texture min mip]
    packet --> create[Sampler lookup and CreateSamplerState]
    create --> submit[PSSetSamplers]
"""#)

`140360bf3/140360d1b` 写入 flags `0x24a`；effect 的虚表槽转到
`140349040`，flags 进入线程记录。`14023c760` 将 flags 写入命令记录，
`14023c83f..14023c85d` 以 texture `+0x56` 和 flags 的 minimum-mip 字段
取较大值。初始化时该字段为零，且 `0x24a & 0x3c0 != 0x1c0`，不走此处
的全局 filter override。

`1402384e0` 的创建路径取 `mode=(flags>>6)&15=9`；`140238618` 的
`mode & 6` 为零，MaxLOD 沿用 MinLOD，两个 float 分别写于
`140238613/14023862a`。所得 D3D11 sampler 为：Filter=`0x14`
（MIN_MAG_LINEAR_MIP_POINT）、AddressU/V/W=`3/3/1`、MipLODBias=0、
MaxAnisotropy=1、ComparisonFunc=1、BorderColor 全零、**MinLOD=MaxLOD=0**。
静态控制值 `0x2ca/0x34a` 走放宽 MaxLOD 的分支，`0x10024a` 固定到 mip 1。

`14023864b` 调用 device 虚表 `+0xb8` 的 CreateSamplerState，device 来源
追到 `d3d11.dll!D3D11CreateDevice` 的 IAT `14210bda8`；`140240bef`
调用 context 虚表 `+0x50` 的 PSSetSamplers。Microsoft D3D11 契约把
MinLOD/MaxLOD 定义为可采样 mip 的上下界，0 为最详细一级。

该关联支持 272 条路径在创建状态下使用完整 base mip。其他状态写入者、
替换/流式生命周期和实际 draw 选择没有由静态锚点穷尽；缺失低 mip 的
上传内容也没有由 sampler 限制确定。单 mip 解码规则本身只依赖容器与 TEX
结构，不以天空文件名或这条消费链作为放宽校验的条件。

## 公开资料、许可证与接触记录

- [TexTools PR #96](https://github.com/TexTools/xivModdingFramework/pull/96)，
  合并 commit `57d1ada3c103e883d8d3790ad0aa83f1a289fd11`，patch head
  `08b01421e4a6153ffce59b7ebfb2e062495ac5ed`。先审 `LICENSE.txt`（GPL v3），
  再读八个 patch 和固定 `Textures/FileTypes/Tex.cs` 的 1–245 行；该文件
  声明 GPL-3.0-or-later。八个 patch 路径及完整响应登记在外部 PROVENANCE。
  `LICENSE.txt` SHA-256 为 `27758f92046551529fa16f818263c5de0ab692dad268a875fa192e075806e4c8`，
  保存的完整 `Tex.cs` 摘要为 `66266c160e07fbd3e770c4b5e907c5cc83a21e01998781edf4e890e1ee67dd1d`。
  二维 mip 修复没有提供 volume 缺失切片来源；代码未复制、翻译或引入库。
- [MicrosoftDocs/sdk-api](https://github.com/MicrosoftDocs/sdk-api/tree/656e9656bfaef34dd29cd7ee9e1146aec17e924c)，
  commit `656e9656bfaef34dd29cd7ee9e1146aec17e924c`。先审根 LICENSE 的
  CC BY 4.0，再使用 D3D11_SAMPLER_DESC、D3D11_FILTER、CreateSamplerState
  和 PSSetSamplers 文档。作者/权利来源为 Microsoft 文档项目；本页字段
  语义为中文归纳，数值来自客户端字节计算。原文和逐篇固定 URL/摘要保留于
  `sdk-api-pinned.json`，LICENSE 摘要为 `7183ef9749eb9b75e710a81a4c812a9405cc184f087500581a973e5929ac3f5e`。
- DXBC token 采用 Microsoft DirectXShaderCompiler commit
  `ea3489109cc8a968b36e36ca20a698a68b516d32` 的声明头与 University of
  Illinois/NCSA notice；固定版本和许可证沿用 [PBD shader 观察](pbd-scalar-consumer.md)。
  只观察资源维度和指令，没有执行 shader。
- 通用解压依赖复用项目锁文件的 flate2 1.1.10 / miniz_oxide 0.9.1；外部
  `dependency-notices.json` 与 `dependency-notices/` 保留六个依赖的全部
  LICENSE/NOTICE/COPYING 和摘要，没有引入 FFXIV 解析库或新的 copyleft 依赖。

既有 [Meddle/Physis 接触记录](external-volume-review.md)完整保留。原型及
整合的参与者有已登记的源码接触，不宣称严格 clean-room；项目发布与许可
决定独立保留。273 条的六级完整重建仍需可核对的缺失内容或生产生成规则。

[缺口的存储与生成证据](volume-generation.md)记录到后继索引条目的完整
物理区间、Texture3D 创建标志、替换复制路径与三维滤波前缀的数值关系。
滤波/量化模型的反例与生产规则缺口独立保留；单 mip 的像素身份及完整性
条件仍由原始组字节决定。
