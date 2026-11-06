# BC6H 来源与验证

## 通用解码依赖与许可

`bcdec_rs` 0.2.0 是通用 BC block 解码器，没有 FFXIV 格式解析、文件系统、
HTTP、GPU 或运行时依赖。本库固定依赖该版本，仅调用它的 `bc6h_half`，
不复制、翻译或照搬其算法到项目。crate 无传递依赖，Cargo 声明 `build=false`。
固定压缩块 16 字节、输出 48 个 u16、行跨度 12 个元素；长度检查、裁边、
图像布局、资源预算和 half→f32 通道输出由本库处理。未改动原有 BC1–BC5/BC7
解码路径。不读取 legacy，不引入 FFXIV 解析依赖，不宣称严格 clean-room。

审查顺序为 crates.io 元数据与下载包身份、仓库 LICENSE，然后才读取 crate
API/源码的 `bc6h_half` 签名、精度/分区声明和 reserved-mode 标记。没有阅读
或借用 FFXIV 解析实现。依赖身份为：

- crates.io `bcdec_rs/0.2.0`，包 SHA-256
  `f09c37bc0e9f0924b7dae9988265ef3c76c88538f41a3b06caf4bed07cee5226`。
- 包内 `.cargo_vcs_info.json` 指向 ScanMountGoat/image_dds
  `be82800d4cdf20375cd1f3dedb662d42e62242a3`、路径 `bcdec_rs`。
- 包的 `Cargo.toml` 声明 MIT，但没有附 LICENSE 文件；在上述固定 commit 的
  根 `LICENSE` 审查 MIT，Copyright (c) 2023 SMG。
- crate 声明为 iOrange/bcdec 的 Rust port。先审查其 LICENSE，固定复核 commit
  `80859ed3b7afb1c527a2a99d70c61457bea72d0c`，MIT OR Unlicense，选择 MIT。
  这是上游许可复核身份，不代表 crate 的原始 C 实现精确来自这个 commit。
  原 C 解码器未作为独立 oracle；同一算法的 C/Rust 比较不构成独立像素证明。

### 保留的 MIT 通知

Copyright (c) 2023 SMG

Copyright (c) 2022 Sergii Kudlai

Copyright (c) Microsoft Corporation.

Copyright (c) .NET Foundation and Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## 官方位格式参考

先审查 MicrosoftDocs/win32 的 `LICENSE`（CC BY 4.0）与 `LICENSE-CODE`（MIT），
再阅读 `desktop-src/direct3d11/bc6h-format.md` 与所引用的两张布局图。
版本复核为 `ca8274decead49403d70ceeed4af8d4ca74eaf11`，文档作者 Microsoft。
来源：[BC6H Format](https://github.com/MicrosoftDocs/win32/blob/ca8274decead49403d70ceeed4af8d4ca74eaf11/desktop-src/direct3d11/bc6h-format.md)，
许可：[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)。项目说明与
工具为自行组织的字段事实和数学实验，不复制官方伪代码的程序结构或图片。
已阅读的图像摘要：

- `bc6h-headers-med.png`：
  `38479285e2e82b82b2379abda62f15ef31bcf72a2d2507413615a55ae01441a4`。
- `bc6h-partition-sets.png`：
  `14fc4ea200a92935072fb20ac19b68cc3616dfff9874077978d520d870109986`。

DirectX-Headers 另先审查根 MIT LICENSE，再读取 `include/directx/dxgiformat.h`
的枚举，复核 commit `faf928dc6feb8b86e9e3d6a39fc5171de7d8ac36`。它明确
BC6H_UF16 为 95、BC6H_SF16 为 96，BC7_UNORM 为 98。没有复制实现代码。

## TEX ID 的符号约定

已授权安装 `2026.10.06.0000.0000` 的固定 Win64 executable SHA-256
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9` 中，
首选 VA `140218100..140218389` 是格式转换分支；`14021831a` 比较
`0x6330`，命中 `140218346` 后返回 `0x60`。相邻 `0x6230→83`、
`0x6432→98` 与 DXGI 枚举相符。因此这里使用 **SF16，有符号 RGB**，
不是仅从零值样本猜测 unsigned 格式。

观察用离线 GNU objdump 2.46，未执行、附加或模拟游戏，不是 REA Evidence。
只保留固定区段摘要与字节锚点，反汇编在外部输出目录。没有移植格式转换函数。

## 实资源与受控实验

ex5 的真实 `256×256`、六面九级 cube 为 524528 字节，type-4 的全部字节与
独立有界 raw DEFLATE 重建一致。所有 32778 个 BC6H 块均为全零，只有一个
独特块，属于官方 mode 1。54 张面/mip 的 RGBA32F 位均为 `(0,0,0,1)`，
RGBA8 均为 `(0,0,0,255)`。这只证明真实容器与零值输出，不证明实文件中
非零 HDR 或所有 mode 的解码正确性。

`verify_bc6h.py` 依据官方 mode 11（单区域、两端点各通道直接有符号 10 位）
构造非零块，端点含 -512、-511、零、正负中间值和 511。首像素的 anchor
索引为三位，其他像素四位；索引覆盖 16 个插值权重。独立 oracle 直接在生成
前的整数端点上计算符号恢复、精度扩展、权重混合与最终 half 位，再由 Python
struct 的 binary16 转为 f32。它不调用 bcdec_rs 或其上游 C 实现。

四个受控 TEX 轮廓包括 `29×31` 裁边二维、三层四级数组、六面四级 cube，
以及三切片三级 volume。连同实际零 cube，共 96 张图像、525911 像素、
2103644 个 f32 分量逐位一致，其中 2398 个负值、950 个大于 1 的值。
所有 RGBA8 输出也按独立 clamp/round 对照。坐标/索引/顺序访问与单图像/
整 mip 输出保持一致，没有通过容差接受不同 half 值。

`tests/fixtures/bc6h-observations.json` 固定 executable、区段、实际资源身份与
浮点 oracle 摘要。受控回归另覆盖 ±65504、alpha=1、裁边与输出预算、截断块、
四个 reserved mode（规范指定黑色），以及全部 32 种低五位前缀下的 2048 个
确定性任意块无 panic。任意块测试不是所有 mode 的独立数值证明。

非零真实 BC6H 和 unsigned TEX 变体仍有证据缺口；库的 `0x6330` 不借此
声称覆盖 unsigned 格式或其他平台映射。多模式 CPU 对照的独立证据见下节。

```sh
nix develop -c cargo build --features local --example extract_entry --example validate_tex
# 使用 TEX 来源页列出的带 Pillow Python 环境以复用独立 type-4 观察器：
python tools/verify_bc6h.py /path/to/game/sqpack /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/bc6h-check
```

## Microsoft CPU 多模式 oracle

来源与许可均先审查再读取内容。Microsoft DirectXTex 与 DirectXMath 根 LICENSE
为 MIT、Copyright Microsoft Corporation；dotnet/runtime 根 LICENSE.TXT 为
MIT、Copyright .NET Foundation and Contributors。具体身份和采用文件的
SHA-256 固定于 `tests/fixtures/bc6h-oracle-sources.json`：

- DirectXTex `c0cb97764379ebd20dbd2016c58965db85f5b307`：读取通用
  `DirectXTex/BC6HBC7.cpp`、`BC.h`，包含解码/编码和 BC7 内容；另读取私有
  umbrella header 的编译上下文，以及 `DirectXTex.h` 的 BC7_QUICK 常量声明。
  编译完整、未修改的 BC6HBC7.cpp，但 oracle 仅调用 `D3DXDecodeBC6HS`。
- DirectXMath `ee612d379528a69d11df7dfb72f5e656e909376a`：使用原始 `Inc/`
  头与 inline 文件，含 binary16/32 转换。构建校验整个 Inc 文件清单摘要，
  定义为按文件名排序后依次加入文件名、NUL 与原始 SHA-256 二进制摘要。
- dotnet/runtime `07b1f05c43023f6352f5479306f9a9673bf2f715`：使用
  `src/coreclr/pal/inc/rt/sal.h`，不运行 .NET 或其工具链。

它们都不含 FFXIV 解析，不是 bcdec_rs 上游的 C 版本；因此该实验与原 Rust
port 的自我对照有不同来源。第三方源码、原始许可证和编译产物保存在仓库外。
oracle 输出目录保留三份许可证副本，MIT 通知亦保留于本页。没有复制或移植
这些解码算法到库，没有添加 Cargo、WASM 或正常测试所需的 C++ 依赖。

`tools/bc6h_oracle/` 仅为自行编写的标准头编译上下文和输入/输出适配器。
它用原始 BC.h/DirectXMath 类型，输入有界 16 字节块流，输出 16 个 RGBA32F
像素；不读取 TEX/SqPack，不创建图形设备，不调用编码器。替代 umbrella header
不修改解码代码、表、算术或 half 转换；补入的 BC7_QUICK 常量仅供未使用编码
代码编译。构建入口不下载、不编辑第三方源码，先验证所有固定摘要，再编译。

`verify_bc6h_modes.py` 的语料由 SHA-256 确定性生成，不从 Rust 解码结果反推。
14 个有效模式与 4 个保留前缀各 256 块；两区域模式的 shape 位 77–81 轮流
覆盖全部 32 种分区，每种八块，其余端点与 index 位变化。另含零/全一主体。
每个前缀独立包成 `4×1024` 的 TEX（256 个顺序块），经公共 RGBA32F API
解码，与 Microsoft 原始块解码像素逐字节比较，不接受浮点容差。

全部 4608 块、73728 像素、294912 分量逐位一致。四个保留前缀均为黑色且
alpha=1；有效模式包含负值和大于 1 的值。该语料证明这些受控块及全部模式
路径，不穷尽 128 位输入，也不冒充游戏中的非零 BC6H。尺寸裁边、数组/cube/
volume 与 RGBA8 的独立实验仍由前述 mode 11 验证承担。

`tests/fixtures/bc6h-mode-observations.json` 固定每个前缀的输入和独立 oracle
输出摘要及统计。带显式 oracle 路径时复算并比较完整字节；普通资源联跑只
重建语料并比较已固定的独立输出摘要，不下载工具或隐式信任其他可执行文件。

取得上述固定版本并保留许可证后，可复现完整 CPU 对照（需 g++）：

```sh
# SAL 目录含固定版本的 sal.h 和原始 LICENSE.TXT；另外两项为固定版本源码根。
python tools/build_bc6h_oracle.py /path/to/DirectXTex /path/to/DirectXMath \
  /path/to/sal-directory /tmp/opencode/bc6h-oracle-build
nix develop -c cargo build --example validate_tex
python tools/verify_bc6h_modes.py /tmp/opencode/bc6h-modes-check \
  /tmp/opencode/bc6h-oracle-build/oracle
# 只复跑固定 oracle 摘要，不需要第三方源码或 C++ 编译器：
python tools/verify_bc6h_modes.py /tmp/opencode/bc6h-modes-digest-check
```
