# SqPack type-3 验证证据

扩展样本与重建后的几何交叉验证见 [MDL 证据](mdl.md)，包括两/三 LOD、八影响蒙皮与场景分类。

## 来源审查

Lumina commit `8802d66a3cb3032be2fcf998bb10e864221bdb13`，WTFPL v2。
许可证先于内容读取，审查记录见 [IMC](imc.md)。
阅读 `Data/Structs/SqPackFileInfo.cs` 中 ModelBlock 字段声明；
阅读 `Data/Parsing/MdlStructs.cs` 的前 6500 字符，包含 ModelFileHeader 与 vertex declaration 定义及其读取方法。
此接触已记录，不宣称严格 clean-room。没有读取 SqPackStream 的模型重建实现。
未复制、翻译或照搬解析代码，未依赖 Lumina。

采用的公开结论包括：208 字节模型条目固定布局、11 段字段编号、逻辑 MDL 的 68 字节头。
真实 DAT 观察验证了字段位置、块范围、存储跨度、对齐容量与实际长度的差异。
所有非空段在 Python zlib 与本库 flate2 上逐块解压一致，最终重建文件逐字节一致。
未以这些一致性结论替代 MDL mesh/index/shape 的独立语义验证。

## 样本身份

来源：只读 `/home/azurice/Games/ffxiv/game/sqpack/ffxiv/040000.win32.dat0`；
Win32；版本文本 `2026.10.06.0000.0000`。

| 游戏路径 | DAT 绝对偏移 | 输出字节数 | 输出 SHA-256 |
| --- | --- | --- | --- |
| `chara/weapon/w2001/obj/body/b0102/model/w2001b0102.mdl` | 8316865536 | 170940 | `66be81af7fbbd368b14052cde9dedcd047efdf12f87e5f522672a10ac9b4b425` |
| `chara/equipment/e0001/model/c0101e0001_top.mdl` | 136767616 | 94836 | `961c5b587b4ed5dd2c51d9ec57764b8761c4a0af57cf2202bb2247db9d47e26d` |

两份条目头均为 256 字节，SHA-256 分别为：

```text
a08b8dbb9a823ab7865a4fd0a63f502b7f8dd5501eec03b163b0b60d5cb8ed6a
1f9a71951d56c0c68333b7a4c6cf229ee7d61ab5745d61e721b5d11a83457c6a
```

## 独立观察

- 武器：stack 实际 408 / 容量 512；runtime 实际 736 / 容量 768。
  三个 vertex 段实际 80280、39920、32840；index 为 11184、3312、2192。
  共 17 块，实际段长度加 68 恰等于 170940。
- 装备：stack 实际 816 / 容量 896；runtime 实际 2240 / 容量 2304。
  三个 vertex 段实际 30888、28080、18520；index 为 5552、5552、3120。
  共 11 块，实际段长度加 68 恰等于 94836。
- 所有非空段的容量均等于实际长度向上取整至 128 字节。
- 两份样本均有三个 LOD，MDL version 为 `0x01000006`；无非空 edge 段。
  非空 edge 段的逻辑位置依据分段关系推断，只有受控测试，不作为已验证游戏变体。
- 头部 `+0x0c/+0x10` 的用途未知；两个样本数值吻合 128 字节单元计数，但未采用为寻址依据。

## 复现

```sh
nix develop -c cargo build --features local --example local_sqpack
python tools/verify_model.py /home/azurice/Games/ffxiv/game/sqpack /tmp/opencode/model-evidence-01 chara/weapon/w2001/obj/body/b0102/model/w2001b0102.mdl chara/equipment/e0001/model/c0101e0001_top.mdl
```

输出目录必须不存在且位于安装之外。脚本只读 index 与 DAT，记录每块的绝对位置、原始长度、
解压长度，保存条目头、重建文件、提取日志与 `report.json`。
初次运行工具：Python `3.14.8`，zlib `1.3.2`；28 块与全部逻辑字节比较通过。
索引探测仅限基础包 `chara` 类别的普通 index，不验证 synonym、index2 或其他类别。

## 回归测试

`tests/sqpack_resource.rs` 的模型用例使用自行构造的 raw/DEFLATE 混合段，
验证物理/逻辑次序差异、全部 11 段、精确 buffer 偏移、实际长度、标志与输出预算。
拒绝重叠段、重复/越界块索引、非法容量、错误跨度、超额解压、短模型头及非法 LOD。
受控 edge 段测试只验证字节保留与偏移计算，不证明其 Win32 格式语义。
