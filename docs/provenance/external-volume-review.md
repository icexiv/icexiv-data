# Meddle / Physis 的纹理读取路径审阅

## 来源与许可

用户于 2026-11-05 明确要求审阅 Meddle、Physis 等项目。审阅用于识别
volume 缺口的处理方式及独立验证线索，不引入依赖，不复制、翻译或移植
解析代码，不宣称严格 clean-room。源码克隆留在仓库外，未运行这些项目、
插件或游戏，未读取本项目 `legacy/`。

- [Physis](https://github.com/redstrate/Physis)，commit
  `23176fcd985cf45a4c745c2284577426c7dde6b4`。先检查 Cargo、REUSE、许可证
  和源文件 SPDX 头。Cargo 声明 `GPL-3.0`，所读 Rust 文件声明
  `GPL-3.0-or-later`，仓库包含对应许可证。REUSE 的 CC0 注解涵盖
  README/Cargo 等元数据，不将解析源文件改为 CC0。
- [Meddle](https://github.com/PassiveModding/Meddle)，commit
  `383bdf82d2e35741039f1474c2c9f475f2cec9f9`。先检查根 `LICENSE.txt`
  （GNU AGPL v3）与 README 来源声明。README 标明 Xande、Penumbra、
  Lumina、SaintCoinach 等来源；本次没有沿这些链接继续阅读实现。
  所读 C# 文件按仓库许可证审阅，不因缺少单文件许可证头而视作无限制。

这些 copyleft 实现仅供经授权的审阅，不作为代码模板或依赖许可决定。
项目发布和许可证决定独立保留。

TexTools PR #96 的固定版本/许可接触、完整单 mip 正向验证与天空初始化
采样链见 [Volume 单 mip 证据](volume-mips.md)。

## 阅读范围与身份

行号对应上述固定 commits。除路径/关键字定位外，实际阅读范围为：

| 项目 / 文件 | 范围 |
| --- | --- |
| Physis `src/resource/sqpack.rs` | 1–15；texture/decompress 等定位搜索无命中 |
| Physis `src/sqpack/data.rs` | SPDX 头、texture 定位搜索、420–470 |
| Physis `src/tex.rs` | SPDX 头、mip/depth 定位搜索、185–287 |
| Meddle `Meddle/Meddle.SqPack/SqPackUtil.cs` | 1–115、265–307；其他位置仅定位搜索 |
| Meddle `Meddle/Meddle.Formats/Files/TexFile.cs` | 1–226 |
| Meddle `Meddle/Meddle.Utils/Export/TextureResource.cs` | 1–23 |
| Meddle `Meddle/Meddle.Plugin/Utils/DXHelper.cs` | 1–125；调用位置仅定位搜索 |
| Meddle `Meddle/Meddle.Utils/Export/Vertex.cs` | 1–190 |

核心被审阅文件的 SHA-256（Meddle 文件路径见上表）：

| 文件 | SHA-256 |
| --- | --- |
| Physis `src/sqpack/data.rs` | `9106c37e4ac452c53363accc40517fac5472fcbe7a346c168d2c0763c0e0ba24` |
| Physis `src/tex.rs` | `2da3fbf3ebc5c5cd487e700f70fcfd5453cdbfdac4a0b5bb754026aab481f897` |
| Meddle `SqPackUtil.cs` | `fdfa6638be8dafef4d4dc5ba3f08a584fab2b339b4d9d1cbd0b5062fa9913200` |
| Meddle `TexFile.cs` | `10c478e271656d3f1d74a3d717b4e47b02999c4126bd3ccb6871a508b3041f76` |
| Meddle `DXHelper.cs` | `924833a6d752d014c34e87f29ad89889ba724f904a2cec366d54b48c04fa6de9` |
| Meddle `Vertex.cs` | `1bb93bcc24c714fd1a27ba4c9d1ddfeef88c7e9a67c01b922d235f100cbe5555` |

## Type-4 返回长度与缺口

以下是指定源代码路径的静态观察，不是对整个项目所有版本的行为声明，
也不是实际调用这些库所得的输出：

- Physis `read_texture_file` 为声明大小预留 capacity，先放前缀，再顺序
  追加各块解压字节并返回。不将 length 扩展为声明大小，不按 TEX mip
  offsets 留空槽，也没有最终总长一致性检查。
- Meddle `ParseTexFile` 建立声明大小的 C# byte 数组，顺序写入前缀和块
  输出。数组初始化为零，写入量不足时尾部保持零；该函数没有按各 mip
  声明起点重新定位目标。零来自工具数组初始化，不是磁盘块提供的值。
- Meddle `TexFile.SliceSpan` 另用 header offset 的差值定位，越过数据
  返回空 span，末尾不足则截短。它不证明顺序拼接与原始 mip 槽一致。
- Meddle `DxHelper.ExportTextureResource` 从客户端 D3D11 2D 资源读取
  数据。该运行中入口与离线解包有不同输入条件；本次没有执行它，不能用
  其存在证明异常 volume 的上传内容或 3D 支持。

## 本地独立复验

已有的 `observe_texture_group_sizes.py` 使用 Python struct、有界 raw
DEFLATE 和本库提取示例复验固定见证；没有调用或重实现外部解析器。
安装为授权的 Win32 `2026.10.06.0000.0000`，身份与工具边界沿用
[TEX 来源记录](tex.md)。

| DAT / 字节位置 | 声明总长 | 前缀与所有解压块总长 | 组输出长度 |
| --- | --- | --- | --- |
| `ffxiv/010000.win32.dat1@967239424` | 28172 | 26284 | 24576,1536,64,16,8,4 |
| `ffxiv/010000.win32.dat0@1609282304` | 37540 | 35052 | 32768,2048,128,16,8,4 |

两者 mip 1 起点与顺序累积位置一致，但组输出分别只有 1536/2048 字节，
其声明槽分别为 3072/4096 字节。从 mip 2 开始，顺序拼接的位置不再等于
声明起点。向整个拼接结果尾部添加零，不能等同于逐 mip 槽内存在已知零值。
此判断只使用本地头部偏移和独立块长度，不将外部实现作为格式规范。

复验报告 SHA-256 为
`7cc5378ae4fcff891d85c66af1193e11dae3c122450026d2fe0803827811d749`，
报告与输出位于 `/tmp/opencode/texture-groups-third-party-review-01/`。
缺口见证明确报长度不一致；同一工具的完整零 BC6H 对照通过。

```sh
python tools/observe_texture_group_sizes.py /path/to/game/sqpack \
  /tmp/opencode/texture-groups-third-party-review
```

## 顶点语义线索与验收

Meddle `Vertex.VertexUsage` 将 raw usage 5 命名为 Flow，注释关联
TANGENT0；usage 6 命名为 Binormal。这是可追踪的消费语义假设，不是由
高 UV index 推断 flow 的依据。本库实际 MDL 中已有 usage 5 的 scalar
数据对照；[客户端顶点输入证据](vertex-inputs.md)另以原生转换路径和实际
SHPK 签名独立核对其 TANGENT0 身份。该关联不来自直接采用外部枚举，
shader 流向公式和额外 index 的消费仍有证据边界。

两个实现的返回策略解释了工具为何可能接受短输出，但均没有提供差额像素
的来源证据。可以针对原生输入声明追踪 usage 5 的命名；volume 完整重建
需要能说明省略区内容与适用条件的独立证据。缺口维持显式失败，不以宽松
返回、零初始化或运行时导出入口替代正确性验收。

## 公开生产线索的隔离调查

`external-volume-sources-17/` 为 2026-11-06 用户授权的隔离调查，只回传
功能存在性、来源、许可和证据等级。固定 Physis commit
`3f1f6534644e1cbbb40c8338856f394450b243bd`（0.8.0，相关文件
GPL-3.0-or-later），Meddle commit
`383bdf82d2e35741039f1474c2c9f475f2cec9f9`（0.1.57.0，GNU AGPL v3）。
许可审查先于实现阅读；实现内容没有回传，不引入这些依赖。

查询范围为 12 次 web 搜索、4 次 GitHub 列表查询、6 个实现文件和 7 条
issue/PR 正文。未找到同类短天空 volume 的精确恢复、前缀存储成因或生产
writer 证据。Physis 的 3D 支持提交
`c4e9697faa4d33436997b17ebeb11553dd8474f8` 的元数据涉及 tiling textures；
[Meddle PR #54](https://github.com/PassiveModding/Meddle/pull/54)涉及 cubemap
导出修复。二者没有给出本题资源身份、原始制作文件或省略区像素。

净化报告与完整查询账本分别为外部 `REPORT.md` 和 `QUERY-LEDGER.md`。
公开资料不能确认作者是否处理过同类输入；这项有界负向结论不证明社区
没有相关资料，也不增加六级 mip 重建通过数。能够推进生产问题的线索是
带资源身份的作者样本、原始制作文件、回归结果或生产工具说明。
