# IMC 来源与样本证据

## 审查

- 需求仓库 commit：`a38d3c9bfb67fb441d17a77b344f77c83058dca0`；阅读 `docs/render-book`，未读取解析实现。
- Lumina commit：`8802d66a3cb3032be2fcf998bb10e864221bdb13`。
  先读取 [LICENSE](https://github.com/NotAdam/Lumina/blob/8802d66a3cb3032be2fcf998bb10e864221bdb13/LICENSE)：WTFPL v2，许可允许审阅。
  再审阅 `src/Lumina/Data/Files/ImcFile.cs` 的字段与说明；获取文件也暴露了方法体，记录此接触，不宣称严格 clean-room。
  Rust 的借用表、长度检查、位编号/rank 查询实现由本项目编写，未复制、翻译或照搬该解析代码，也不调用 Lumina。
  采用的格式结论：4 字节头、6 字节记录、默认与变体分组、字段名称、attribute/sound 位宽、动画低 nibble。
  Lumina 的 SoundId getter 保留位位置；本库 getter 返回右移后的六位 selector，此为 API 表达差别。
- xivdev/docs：GitHub 元数据 `license: null`；根目录 `LICENSE` 在 main/master 均为 404，匿名 API 返回过限流。
  未使用其 IMC 正文或实现，未把仓库既有致谢当作许可审查。

## 安装与样本

平台：Win32；安装版本文本 `2026.10.06.0000.0000`；来源为用户授权的
`/home/azurice/Games/ffxiv/game/sqpack`。游戏资源不提交或再分发。

| 游戏资源路径 | 字节数 | 变体数 / 部件掩码 | SHA-256 |
| --- | --- | --- | --- |
| `chara/weapon/w2001/obj/body/b0102/b0102.imc` | 16 | 1 / `0x0001` | `61e413bf765cc2ac7d4ac604594fccbe133641062788ec33ea691a9ad2a8324f` |
| `chara/equipment/e0001/e0001.imc` | 1804 | 59 / `0x001f` | `37959f37755166289565014945fe495866c68ec5c838a84bbd1eb5dce5cdad51` |
| `chara/equipment/e0600/e0600.imc` | 154 | 4 / `0x001f` | `1800d6b2cd09586090bf9710109cef056c5953e64e3804f78dc273d7f8568ffa` |

## 复现

在仓库根目录执行，输出目录需为安装之外且不存在的新目录：

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_imc
python tools/verify_imc.py /home/azurice/Games/ffxiv/game/sqpack /tmp/opencode/imc-evidence-01
```

脚本通过 SqPack 读取样本，独立使用 Python `struct` 顺序观察所有记录，与 Rust 查询输出逐行比较。
输出包括提取字节、提取日志、`rust.tsv`、`report.json`。报告记录 Python 版本与每个样本哈希。
初次运行 Python `3.14.8`，327 条记录全部一致；三个文件长度完全符合分组公式，无尾部填充。
这验证真实样本字节与表结构，不把两套解码使用相同字段假设当成播放语义的证明。

武器默认记录 packed mask 为 `0x03ff`，变体 1 为 `0x33ff`；低 10 位相同，高位 selector 分别为 0 与 12。
`e0600` 可观察到低位值 `0x3fd/0x3fe/0x3ff` 与高位值独立变化。
`e0001` 在多个子集内呈现同组材质 ID 同步递增，支持子集优先而非部件优先的记录次序判断。
三个样本的 decal、VFX、动画字节均为零；没有据此证实其非零引用解析、动画 nibble 的游戏行为或未知高位语义。
这三份文件不作为非零字段的证据；非零记录与字段 getter 的独立对照见下文。

## 回归边界

`tests/imc.rs` 包含 16 字节真实记录的最小数值转录、逐前缀截断、尾部字节、空掩码、极端计数、
缺失 part/subset、稀疏及高位 part mask、原始动画高位保留和确定性任意输入测试。
稀疏/高位掩码是受控 API 边界测试，不证明游戏使用这些变体。

## 装备与配饰的非零字段

`find_imc_variants.py` 按实际资源路径模式查询 ID `1..9999` 的 equipment 与
accessory IMC，共 19998 个候选路径，仅查固定角色主索引
`ffxiv/040000.win32.index`。索引表 SHA-256 为
`3570cbbd52654e524629b3e083669b6d844e5c251a88a9c655a988700abff051`。
每文件解码上限 1 MiB、最多 5000 个实际文件、累计解码上限 64 MiB；选中的
1812 个实际文件共 329388 字节，未耗尽这些文件/字节预算。路径范围不等于
整个安装或全部武器/怪物目录，不将未查询到的路径当成安装层的全局缺失。

工具对每个定位使用 Python struct 与有界 raw DEFLATE 独立重建完整 type-2
文件，核对全部子集与部件的尺寸公式。`verify_imc_variants.py` 重复这些身份
和输入观察，随后以 Rust SqPack 路径查询读取同一批文件并解码全部记录。
比较包括头部 count/mask、逐子集/part bit、全部六字节字段、attribute/sound
分拆和 material-animation 低 nibble，共 53690 条记录逐项一致。验证示例使用
有容量界限的索引缓存，缓存不改变缺失/IO/解析错误语义。

实际记录中有 82 条非零 decal、71 条非零 VFX、2 条非零动画字节；动画字节
高 nibble 在此范围为零，不由此推断高位非法或不存在。具体见证为：

| 资源 | 子集 / part bit | 字段数值 |
| --- | --- | --- |
| `chara/equipment/e0002/e0002.imc` | 179 / 1 | material 50、decal 201、packed mask 3071、VFX 0、animation 0 |
| `chara/equipment/e0137/e0137.imc` | 1 / 1 | material 2、decal 0、packed mask 7167、VFX 0、animation 10 |
| `chara/equipment/e0137/e0137.imc` | 2 / 1 | material 1，其余同上 |

e0002 完整文件 5554 字节、184 个变体、mask `0x001f`，SHA-256 为
`d8f07e9e7e6b5af25c00caee926294b0920780c318110379521e4425ebe2f2cb`；
e0137 完整文件 94 字节、2 个变体、mask `0x001f`，SHA-256 为
`ea747bc16d958ca93c912bfed36e6fdff6c11b6f9901b228e55dfc38ec126110`。
全部实际文件身份、非零见证和两侧字段输出留在外部报告目录，报告摘要固定在
`tests/fixtures/imc-variant-observations.json`。完整报告 SHA-256 为
`9c0bd47a2cbdc016151a5b344d713ea63fe51ac5538e3b5a66cd9e5281a6c1ef`。

Rust 回归另以这些见证的数值字段构造单部件小表，覆盖非零 decal/动画与
独立 attribute/sound 字段，再遍历全部 256 个 raw animation 值确认高位
保留与低 nibble 查询。小表是本项目的受控产物，不冒充完整实文件。

这组证据解除的是非零字段缺少实际记录和逐字段对照的缺口，不验证材质动画
播放、decal 图像应用或完整资源路径回退。离线检查指定 executable 的路径
字符串还见 `chara/common/texture/decal_equip/-decal_%03d.tex`，其引用过程
`14044b780..14044b7dd` 读取六字节对象数组的第二字节或另一个扩展对象字段。
executable 身份同 [EQP 静态证据](eqp.md#客户端离线静态证据)，上述 93 字节
区间 SHA-256 为 `bd31f102cda451808743328a3f6b851692745c5026bc2418be512459fe14073f`；
使用 GNU objdump 2.46 离线读取，没有执行、附加或模拟目标。
该对象数组与 IMC 文件输入的完整来源链未由此局部观察建立，未把格式字符串
与候选 ID 的组合当作完成引用解析或 native 运行证明。没有新增第三方来源、
移植客户端过程或执行游戏。

```sh
nix develop -c cargo build --features local --example validate_imc
python tools/find_imc_variants.py /path/to/game/sqpack /tmp/opencode/imc-discovery
python tools/verify_imc_variants.py /path/to/game/sqpack /tmp/opencode/imc-variants
```
