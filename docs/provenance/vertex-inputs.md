# MDL 到 Win32 顶点输入的证据

## 来源、身份与方法

需求来自 xiv-companion `geometry.not` 的可选 flow/normal/binormal 输入。
[Meddle 审阅](external-volume-review.md)提供 usage 5 → Flow/TANGENT0 的
调查线索；其 AGPL 来源不作为本库代码模板或格式规范，没有移植其实现。
以下对应关系通过客户端自身的离线指令和授权实际资源独立观察。

安装为 Win32 `2026.10.06.0000.0000`。程序是授权只读的
`game/ffxiv_dx11.exe`，51910400 字节，SHA-256
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`。
首选 image base 为 `0x140000000`；以下 native 地址为首选 VA。
`.text` RVA/file 起点为 `0x1000/0x400`，`.rdata` 为
`0x210b000/0x2109800`。脚本核对 PE section，而非将这些差值套用于其他程序。

工具为 GNU objdump 2.46、Python struct/hashlib 和本库提取/验证示例。
REA 原生引擎的环境限制沿用 TEX 来源记录；这是离线 objdump 证据，
不是 REA Evidence 或运行观察。没有执行、模拟、附加客户端或运行 GPU 测试。
没有阅读其他第三方解析实现或 `legacy/`。

## 声明转换与输入名

- `1403207e0..1403209fc` 按声明处理至多 17 个 element，源 element 步长
  8，源声明步长 `17×8=136`。它从 `+0/+1/+2/+3/+4` 分别读取 stream、
  offset、type、usage、usage-index，stream `0xff` 终止。目标 element
  步长为 4，分别存 stream、offset、内部 type、内部 semantic。
- `140320929..140320979` 的 usage 分派表位于 `140320a48`，八个目标
  分别为 `140320940/947/94e/955/95c/962/969/970`。usage 0/1/2/3
  写内部编号 0/1/7/2，usage 5/6 写 14/15；usage 4 为 index+8，usage 7
  为 index+3。usage-index 在 `1403208a4` 读取，仅 UV/color 分支使用它。
- `140320998` 将四字节目标数组传给 `1402261f0`，该包装尾跳
  `140236000`。缓存路径按 element count×4 比较/复制输入，在对象 `+0x20`
  保存数组、`+0x28` 保存 count；它不重新定义 semantic。
- `14021c480` 从对象取数组，`14021c501` 读取 element 的 `+3`，在
  `14021c50d` 调用 `14021c290` 查询名称/index。后者的 17 项分派表
  `14021c408` 将编号 14/15 分别送至 `14021c3c8/3dc`，返回
  `TANGENT`/`BINORMAL` 字符串及 index 0。编号 8–13 返回 TEXCOORD0–5，
  编号 3/4 返回 COLOR0/1，编号 2 返回 NORMAL0。
- descriptor 数组按 32 字节推进，包含名称、index、格式、stream/offset 等；
  后段把数组、shader 字节与长度交给 device 虚表 `+0x58` 的调用。
  这里的静态参数链不声称实际创建了某个输入布局。

`tests/fixtures/client-vertex-input-observations.json` 固定五段 native 字节
摘要、两张分派表及锚点。复验还解析每个名称分支的 RIP 相对字符串地址和
输出 index，防止只凭同名字符串存在就推断对应关系。不移植转换函数、
缓存、fallback 输入或图形对象构造。

## Float1–Float4 的 scalar 身份

`mdl-scalar-compatibility-18/` 固定重取上述 declaration conversion、forward、
cache、input descriptor 四个窗口，共 2049 字节；每个范围与
`client-vertex-input-observations.json` 的旧摘要相同。原临时目录
`/tmp/opencode/client-vertex-inputs/` 缺席，这份捕获不恢复旧完整报告身份。
没有进行 incoming-reference 查询；沿既有正向边补充的两个 callee 窗口
合计 681 字节，额外数据区为零。

对四个选定 raw type，独立静态审核得到：

| MDL type | 内部 type | descriptor 整数 | DXGI_FORMAT | 元素字节数 |
| --- | --- | --- | --- | --- |
| 0 | 17 | 41 | R32_FLOAT | 4 |
| 1 | 18 | 16 | R32G32_FLOAT | 8 |
| 2 | 19 | 6 | R32G32B32_FLOAT | 12 |
| 3 | 20 | 2 | R32G32B32A32_FLOAT | 16 |

声明记录的 scalar 读取位于 `14032089f`，原始分派表为 `1403209fc`；
所选分支写紧凑记录的 type 字段。转交边为 `140320998→1402261f0`、
`1402261f4→140236000`。descriptor 路径在 `14021c512` 读取内部 type，
经 `14021c714` 的 byte selector 与 `14021c6d4` 的 RVA 表选择数值，
`14021c5c5` 写入 32 字节 descriptor 的 `+0x0c` 字段。内部 type 和
DXGI 数值分别核对，不按一般 Direct3D 枚举顺序推定 MDL ID。

DXGI 数值、分量数及 IEEE 754 binary32 定义取自 Microsoft 的
[DXGI_FORMAT 文档](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/656e9656bfaef34dd29cd7ee9e1146aec17e924c/sdk-api-src/content/dxgiformat/ne-dxgiformat-dxgi_format.md)，
commit `656e9656bfaef34dd29cd7ee9e1146aec17e924c`，
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)。许可证审查先于内容；
原文未修改，62498 字节、SHA-256
`610417406ea9acf0c1c33fc4d35209cb20eb534e41d93c86c0e038a584888566`。
中文说明为本项目解读；原文和完整许可保存在外部 `sources/`。
Lumina 固定 commit 的 40 行声明也经重新审查 WTFPL v2 后核对，仅声明
Single3/Single4，不提供 type 0/1 的身份依据。隔离 legacy 审计只提供
功能存在性线索，其实现没有回传或移植。

此证据为有界静态兼容身份：selector 应有 67 字节，窗口仅含前 60 字节，
所选索引 7–10 完整。复制/比较 helper 本体、全部 cache 分支及实际 COM
身份没有在四窗口内验收；同形对象字段和参数支持转交解释，不视为完整
执行证明。4 个窗口的 1780 代码字节、232 表字节和 37 填充字节分别记账，
不将反汇编器对内联数据生成的文本计为执行指令。

独立 `review/verify.py` 对原字节、表项、官方段落及转交锚点复算，完整结果
206365 字节、SHA-256
`c6a2be0c0d55ab661eb8d81622649688e5c529700e92f90ae3d03e71554a5587`。
默认入口逐对象、逐字节只读比较，并检查输入前后 hash/stat 稳定；
拒绝优化 Python 和覆盖已有结果。受控数值与实际模型回归见
[scalar 查询验证](mdl.md#binary32-布局与位保留验证)，真实 type 0/1 文件覆盖
仍是独立样本边界。

## SHPK 输入签名

七份实际 SHPK 的来源与完整身份沿用[材质来源记录](materials.md)，并在
本项 fixture 再固定摘要。`verify_client_vertex_inputs.py` 只读取各 vertex
shader 的 DXBC 边界和 ISGN chunk；同批完整 SHPK 元数据/字节边界由既有
Python 与 Rust 对照独立核验。

观察 profile 为 ISGN：8 字节 count/record 起点，随后每条 24 字节，含名称
相对偏移、semantic index、system value、component type、register、两个
component mask 字节及保留位。工具检查 chunk 长度、记录范围、字符串终止、
mask 子集、保留位和每个 vertex shader 的唯一输入签名，不解析或执行指令。
这是固定 profile 的调查工具，不扩充本库为通用 DXBC parser。

共观察 588 个 vertex shader、4490 条签名记录：

| SHPK | Vertex shader 数 | TANGENT0 声明数 | 非零读取 mask 数 |
| --- | --- | --- | --- |
| character | 176 | 32 | 32 |
| characterlegacy | 64 | 0 | 0 |
| skin | 72 | 0 | 0 |
| bgcrestchange | 9 | 0 | 0 |
| water | 19 | 0 | 0 |
| bg | 56 | 8 | 4 |
| hair | 192 | 32 | 32 |

72 个 TANGENT 声明均为 index 0、四分量 mask。character/hair 的读取 mask
为 xyz；bg 的四个为 xyzw、另四个为零。签名记录不证明一次运行实际读取、
具体 MDL 与 permutation 的匹配或各分量的流向公式。

完整报告 SHA-256 为
`9d1e742828394ac4e7ad49a1c7986b5d6ebc610b84252897f68b781d33ec46e4`。
逐 shader 签名和反汇编留在外部输出目录。报告与 fixture 不分发 shader 字节。

## 实际 MDL 与 API 边界

[四份 UV 扩展 MDL](mdl.md#uv-高-index-与混合-scalar-type-的实际文件)中的
前三份包含 usage 5/index 0、type 8 的实际声明。其既有逐语义查询文件分别
含 100、202、3624 个有效 usage 5 顶点查询，共 3926 条。全部 normalized
标量已经由 Python 直接按 stream/stride/offset 解码并与 Rust 逐位对照；
这些数据不是只把 usage/index 改名的受控文件。第四份没有该声明。

独立 native 关联确认了 usage 5 的 TANGENT0 输入身份，允许把这组实际数据
称为 tangent/flow 的编码输入，不只称作未知 raw usage。高 index UV 仍不
改名为 flow。受控 Rust 回归另检查 usage 5 与 6 的分离、四标量/alpha 保留、
不做 signed 重映射或方向归一化，以及 raw 非零 index 的独立查询/缺失。

实际 flow1/normal1/binormal1 声明覆盖和 shader 消费公式仍是独立边界。
这条固定客户端转换路径没有把这些 raw 高 index 转成对应的第二组图形输入；
原始数据保留不等于原生输入等价。结论不推广到所有平台、版本或其他路径。

```sh
nix develop -c cargo build --features local --examples
# SHPK 由资源联跑按 shader/sm5/shpk/<name>.shpk 提取：
python tools/verify_client_vertex_inputs.py /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/client-vertex-inputs \
  /path/to/character.shpk /path/to/characterlegacy.shpk /path/to/skin.shpk \
  /path/to/bgcrestchange.shpk /path/to/water.shpk /path/to/bg.shpk /path/to/hair.shpk
```
