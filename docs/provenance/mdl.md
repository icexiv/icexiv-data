# MDL 来源与验证证据

## Glass mesh 实样本调查

`tools/find_mdl_glass.py` 只读授权安装 `2026.10.06.0000.0000` 的 bgcommon/bg
Win32 indexes，以每 index 最多 100000 条、合计 500000 条的界限均匀选择
记录。对选中的 type-3 条目只展开 runtime segment：单段容量上限 1 MiB，
累计容量预算 256 MiB；按真实 u16 span 和块头长度校验 raw/DEFLATE。此工具
不展开全部 vertex/index 段，因而不把头部调查描述为完整 MDL 几何对照。

调查覆盖 21 个 index、500000 条记录、78581 个 runtime 区，没有超出单段
预算的 runtime，也没有耗尽累计容量预算。沿字符串池长度、56 字节模型头、
ElementId 数量和三条 LOD 记录定位可选 extended LOD 表；对声明该表的文件，
检查每个实际 LOD 的 Glass 起点/数量对，未找到数量非零的样本。边界到达的是
选择记录总数上限，不是整个安装扫描完毕或证明该分类不存在。

`tests/fixtures/mdl-glass-discovery-observations.json` 固定安装版本、检查数量与
完整报告摘要；报告逐包包含 index table、所选 entry header 和已观察 runtime
摘要。复跑对照这些记录，只算 inventory observation，不增加完成几何回归
的计数。Glass API 的受控范围测试与真实非空分类证据分别验收，不以零计数
样本代替非空 Glass 解码证明。该调查没有阅读第三方实现、猜测 shader 分类
或执行安装中的程序。

```sh
python tools/find_mdl_glass.py /path/to/game/sqpack /tmp/opencode/mdl-glass-inventory.json
```

## 来源与许可

需求来源为 xiv-companion commit `a38d3c9bfb67fb441d17a77b344f77c83058dca0` 的
`docs/render-book/{geometry,materials,skeleton-animation,vfx-assets}.not`。
只读取需求文档，不将消费侧解析实现作为模板。

外部来源为 Lumina commit `8802d66a3cb3032be2fcf998bb10e864221bdb13`，WTFPL v2；
许可证审查先于内容阅读，见 [IMC 许可记录](imc.md)。审阅范围：

- `Data/Parsing/MdlStructs.cs`：375 行的结构定义及随附字段读取方法。
- `Data/Files/MdlFile.cs`：`LoadFile` 之前的成员声明，不读取加载方法。
- `Models/Models/Vertex.cs`：40 行的类型/usage 枚举与数据成员。

这些内容提供字段名称、旧布局声明和部分 scalar ID。真实样本用于验证字段位置、长度和引用。
本库采用借用表、受检范围、带预算的交叉验证及按需 scalar 解码；没有复制、翻译或照搬第三方解析代码。
审阅暴露了随附读取方法，不宣称严格 clean-room。没有读取本项目 `legacy/`，没有引入现成解析库。

## 声明身份与便捷查询的受控验证

`verify_mdl_semantics.py` 使用下述 Win32 C0 实际文件的第一个 LOD0 mesh，
2007 个顶点；源 SHA-256 为
`66be81af7fbbd368b14052cde9dedcd047efdf12f87e5f522672a10ac9b4b425`。
其实际声明涵盖 position、blend weights/indices、normal、bitangent、color
和 UV。除原件外，工具在仓库外建立七份受控副本，只修改第一份声明中的
usage 与 usage-index 字节，不修改任何顶点、索引或 runtime payload：

- 将两个不同物理输入重标记为同一 usage 的 index 0/1，检查独立查询；
- 将它们重标记为同一 `(usage,index)`，检查查询歧义；
- 分别产生重复 weight、重复 index，以及缺少 weight、缺少 index 的查询；
- 两种 skin 输入均移到其他 usage，检查确认不存在 usage-index-zero pair。

Python 直接用 mesh stream offset/stride 和 declaration offset 计算每个
实际顶点的地址，以 struct binary16/binary32 和 u8 数学解释原有 payload。
查询结果按 `(usage,index)` 分组，不根据 Rust 的查询结果选取源 element。
所有有效标量位、归一化权重位、palette-local indices 与边界/歧义/不完整/
不存在状态，共 138552 条查询行，与 `validate_mdl_semantics` 逐字节一致。
每个副本另确认第一份 136 字节声明之后的源字节完全不变。

这项实验验证的是声明身份保留和库的严格查询契约，不把重复语义称为格式
禁止项，也不证明游戏实文件使用了这些重复声明。原始 declaration 与显式
element 解码仍可使用。实际 normal/bitangent/flow 的高 usage-index 输入和
非空 Glass 样本的证据边界独立保留；颜色多 index 的实际文件见下文。
没有读取新增第三方来源、模拟 native 或执行游戏。

`tests/fixtures/mdl-semantic-query-observations.json` 固定源身份、查询计数和
完整报告 SHA-256；语义行与副本留在外部输出目录，资源联跑复算全部查询。
回归测试另检查歧义不波及其他 usage 的查询，以及无输入时非法 vertex
不能被误报成 `None`。这些错误行为属于 API，不作为 MDL 文件结构事实。

```sh
nix develop -c cargo build --example validate_mdl_semantics
python tools/verify_mdl_semantics.py /tmp/opencode/weapon.mdl \
  /tmp/opencode/mdl-semantic-query-check
```

## Binary32 布局与位保留验证

Type 0/1 的 float1/float2 身份来自[客户端固定窗口与官方 DXGI 规格](vertex-inputs.md#float1-float4-的-scalar-身份)。
活动实现使用已有的受检小端 binary32 读取；返回八个 lane，未使用 lane 为
正零，存储的负零、subnormal、无穷和 NaN payload 原样保留。此为 CPU
解析契约，不推断 GPU 对非有限值或 subnormal 的运行行为。

`mdl-scalar-compatibility-18/verify_scalars.py` 只重取 C0 已登记的固定 DAT
条目，17 个压缩块与 170940 字节逻辑模型独立重建一致。三个 LOD/mesh 的
3826 个顶点、8328 个 index、全部元数据行和完整 scalar/index fingerprints
与既有 `mdl-observations.json` 相同；首 mesh 的 18072 条语义查询亦逐字节
一致。实际文件使用 type 5/8/14，不计为 float1/float2 生产见证。

受控输入共八份：type 0/1/2/3 各两份，只替换第一份声明与首 LOD 的所选
顶点流。所选 stream 为 1、stride 为 24，分别从元素 offset 0/1 读取，
每份保留 2007 个顶点。每种 type 覆盖 2048 种位模式：两个 sign、全部
256 个 exponent 与 mantissa `0/1/0x400001/0x7fffff` 的笛卡尔积。
Python 直接读取无符号整数位，避免 Python float 转换将 signaling NaN
静默化；Rust 使用公共 `Geometry::attribute` 查询。

两侧完整 48192 行、各 2189672 字节相同，包括八个返回 lane、缺少语义和
顶点越界状态。原文件的声明以外区域以及所选流以外的 payload 均保持原样。
这套语料覆盖规定的 2048 种位模式，不声称穷尽全部 binary32，也不将八份
受控副本计为游戏生产样本。

Rust 回归另检查单次解码所有短长度、相邻字节、两个 usage-index 的独立
输入、bulk 输出预算，以及宽度跨越 stride 的拒绝。14 项 MDL 回归及
`just check-all` 的 feature 组合、原生测试和 WASM 编译通过。

完整数值报告为 5348 字节，SHA-256
`5286351aec6a288577a9faf5f85c57b5714a01a4c0c24a96104674746e1a7d1c`；
默认复跑比较完整 JSON 对象、规范字节和全部查询输出，核对 43 个输入的
前后稳定性。首版复跑的 JSON 整数键/字符串键比较错误及原产物保存在外部
`development/attempt-01/`；修正仅统一 JSON 值域，不排除结果字段。
范围、来源、固定窗口与报告身份登记于
`tests/fixtures/mdl-scalar-compatibility-observations.json`。

```sh
cargo build --locked --features local --example extract_entry \
  --example validate_mdl --example validate_mdl_semantics
python3 -B ../icexiv-data-evidence/mdl-scalar-compatibility-18/verify_scalars.py
python3 -B ../icexiv-data-evidence/mdl-scalar-compatibility-18/review/verify.py
```

## Color1 与同 usage 多 index 的实际文件

只读安装来源与版本沿用下述样本身份：Win32 `2026.10.06.0000.0000`。
`find_mdl_semantic_variants.py` 设定每 index 至多 100000 条、合计 500000 条
记录、每个 stack 1 MiB 与累计 stack capacity 256 MiB 的界限，只展开 type-3
的 stack/declaration 段。它在 `ffxiv/010000.win32.index` 的 468 条记录中
观察 71 个 stack，取得四个高 usage-index 候选即停止。没有因此声称枚举完整
模型目录、读取完整几何或覆盖其他高 usage-index 语义。

该索引表 SHA-256 为
`d99e5cbf7b49db58200a28ee5bc8aa4b5b259d620d98211e85ca6c2054e46313`。
发现报告摘要为
`f54b6aa31796286b5a3ee6e4809f7e156d79e5bcacf66620b6ca6ec20cc6ccb7`。
四个固定候选未恢复原始路径，保留 index ordinal/hash 与 DAT 身份，不以
材质名猜测其模型文件名：

| Index ordinal / hash | DAT / 字节偏移 | 完整 MDL SHA-256 |
| --- | --- | --- |
| 285 / `01403abcbe645686` | dat0 / 109501312 | `a5f48bf1254cf10cb598169d17d678171a3d0c956fed28f09166ab58d679bd3e` |
| 346 / `01a3224e3574d721` | dat1 / 1056157952 | `4a5b6f2c14485bcc0b1fb8b0d1e20aa3576d56c057e43c302ef3681b4cd1e0c6` |
| 464 / `024cedfb27546268` | dat0 / 1540143360 | `ae666bc4d2398e20b4ef8381898ab923809c315079d2205883ab7c5a329f66c0` |
| 467 / `024cedfb9864dc09` | dat1 / 962715904 | `87e7428a1389bb65e7d4904c1ab409b640c71d9cea5ddca9877e48cf019709f2` |

DAT 名均为 `ffxiv/010000.win32.datN`。完整验证由
`verify_mdl_semantic_variants.py` 执行：先确认 index 记录，再以 Python struct /
有界 raw DEFLATE 重建所有 type-3 段和逻辑 MDL，逐字节比较 Rust 提取；使用
独立几何 oracle 比较所有已声明 LOD 的普通 geometry，包含全部顶点声明、
scalar、index、材质/骨/shape/attribute/ElementId 等已有语义行。该批全部位置
均在其声明 bounds 内；未据此把 bounds 包含性增加为格式有效性的条件。

这四份文件共 28 个 LOD/mesh 记录、13256 个顶点记录与 35799 个 indices。
其中四个 mesh 声明 color1 而没有 color0；六个 mesh 同时声明两者，4854 个
顶点的两组值不相同。颜色输入均为 usage 7/type 8，其各自的 `usage_index`
分别为 0/1。输入组缺少 index 0 是实际出现的结构，不能自动将 index 1 重命名
或复制成 index 0；双颜色输入也不能被只按 usage 查询时的第一条覆盖。

独立 oracle 按每个实际 mesh 的 stream/stride/offset 解码全部请求。Rust 的
`validate_mdl_semantics --all-meshes` 对所有普通 mesh 查询每个声明语义，
并显式查询 color0/color1、缺失语义与 vertex 越界；全部 104273 条查询结果
（含 mesh 标识行）逐字节一致。既有受控工具的首 mesh 输出模式保持可复验。
小型 Rust 回归另以两组不同 normalized byte 值检查独立查询，以及只有
color1 时 color0 返回缺失而不产生回退。

完整结果固定在 `tests/fixtures/mdl-semantic-variant-observations.json`，报告
SHA-256 为 `91cde5c4a130c00c627fae2512d07ffdb94793b86e154b4676a71ed4dc340ded`。
原始字节、逐块重建记录和两侧输出均保存在外部目录。这是颜色多 index 的
实际数据/查询证据，不推定普通颜色乘法、flow、第二套 normal/bitangent 的
shader 意义，也不解除这些其他语义的实样本边界。没有新增第三方来源、依赖、
游戏执行或 native 仿真。

```sh
nix develop -c cargo build --features local --example extract_entry \
  --example validate_mdl --example validate_mdl_semantics
python tools/find_mdl_semantic_variants.py /path/to/game/sqpack /tmp/opencode/mdl-semantic-discovery.json
python tools/verify_mdl_semantic_variants.py /path/to/game/sqpack /tmp/opencode/mdl-semantic-variants
```

## UV 高 index 与混合 scalar type 的实际文件

同一授权 Win32 安装 `2026.10.06.0000.0000` 中，declaration 调查的
`--extended-inputs` 筛选仅接受非颜色的高 usage-index，或 raw usage ID >=8；
筛选名称不赋予候选 flow 等 shader 含义。其余边界沿用上节的 500000 条总
记录、100000 条/index、1 MiB/stack 与 256 MiB 累计 capacity。两个 index
共选择检查 77617 条记录、11300 个 stack 后取得四个候选即停止；未覆盖全部
模型或证明 normal1/bitangent1 不存在。发现报告 SHA-256 为
`8552ccdfef92a9b23d6e8d668538aec16c8a6c0ad35b2806dea0d45f4993897e`。

`mdl-extended-input-observations.json` 固定如下匿名索引定位和完整资源身份，
不以材质名恢复或猜测原始 MDL 路径：

| Index / ordinal / hash | DAT / 字节偏移 | 完整 MDL SHA-256 |
| --- | --- | --- |
| `010000` / 23516 / `50839fddbbac73dd` | dat1 / 1043472512 | `419fe598965c429a67a6d44a04de02fc0e98935166b20b99c2b10bfebbab0ddc` |
| `010000` / 35754 / `7303de8335346a96` | dat1 / 224851072 | `f5e7ac14d31d9502a5f8f2e6585e48cb885b2d4d4c834beafe3003423bf3ce2d` |
| `010000` / 57934 / `bece785a444b43ce` | dat1 / 978169728 | `36b99cfe262e70409bd221166ca919a52fdf34ebb8f558746b88f2a7c0dfdbec` |
| `020000` / 466 / `01119e63575f6a3c` | dat1 / 852543616 | `ce900f29dcbeb553a0b5455893414227972fd21ca29b8dca38d13a00523b94a1` |

Index 和 DAT 均在 `ffxiv/` 下，文件名分别使用 `.win32.index` / `.win32.dat1`。
前三份模型中实际出现 usage 4 的 index 0、1、2：0/1 为 type 14 half4，2 为
type 13 half2。第四份的部分 mesh 只有 index 0 与 5：0 为 type 13 half2，
5 为 type 8 normalized byte4。这说明不能把 usage 4 的 scalar type 固定成
半精度，也不能只按两个连续 UV index 取数据。前三份还出现 usage 5/index 0
的 normalized byte4；其具体 shader 作用没有仅凭这个编号命名。

完整核对复用独立的 type-3 逐段重建、几何 oracle 和语义查询工具，使用这组
固定 manifest，不修改源 declaration。四份共 51 个 LOD/mesh、91404 个顶点
记录、211863 个 indices；全部元数据、scalar、index 和 735856 条查询输出
（含 mesh 标识）与 Rust 逐字节一致。原有 color1 缺失/双通道对照也继续运行。
小型 Rust 回归使用实际声明形状与自行构造的正/负 half 值、normalized byte
值，检查三组 UV 的独立宽度、index 5 的读取与中间 index 的明确缺失。

完整报告 SHA-256 为
`3dedb3a10e4dd347f40ba02625c649d8c598f0e48c0b1411aff978665ecefd56`。
原始 MDL、逐块记录、逐字段与查询输出均留在外部目录，没有新增第三方实现
来源、依赖或 native 运行。这是 raw UV 多 index 的实际数据证据，不将其等同
于 normal1/bitangent1/flow 的字段命名、shader 公式或材质采样策略证明。

```sh
python tools/find_mdl_semantic_variants.py /path/to/game/sqpack \
  /tmp/opencode/mdl-extended-discovery.json --extended-inputs
python tools/verify_mdl_semantic_variants.py /path/to/game/sqpack /tmp/opencode/mdl-extended-inputs \
  tests/fixtures/mdl-extended-input-observations.json
```

## 版本与样本身份

来源：用户授权的只读 `/home/azurice/Games/ffxiv/game`，Win32；
`ffxivgame.ver = 2026.10.06.0000.0000`。十份样本的 MDL version 均为 `0x01000006`，edge 段为空。

### 已知游戏路径

| 编号 | 游戏路径 | SHA-256 |
| --- | --- | --- |
| C0 | `chara/weapon/w2001/obj/body/b0102/model/w2001b0102.mdl` | `66be81af7fbbd368b14052cde9dedcd047efdf12f87e5f522672a10ac9b4b425` |
| C1 | `chara/equipment/e0001/model/c0101e0001_top.mdl` | `961c5b587b4ed5dd2c51d9ec57764b8761c4a0af57cf2202bb2247db9d47e26d` |
| C2 | `chara/human/c0101/obj/face/f0001/model/c0101f0001_fac.mdl` | `09ee35287ae0ea7f07719ed4c7d09491d9a6309edb78352a6ea21bdd48450f89` |
| C3 | `chara/equipment/e0600/model/c0101e0600_top.mdl` | `6491e675a2a14533e5c87e605d679e578394b14f6d71a2ca929794d1956e7104` |
| C4 | `chara/human/c0101/obj/hair/h0001/model/c0101h0001_hir.mdl` | `1a2d0e1225ac707571de85368dbb83a02312619f5a4b534aa01169aef4900c47` |

C0/C1 的 DAT 位置见 [type-3 证据](sqpack-model.md)。C2/C3/C4 位于基础包
`040000.win32.dat0` 的 `8961282176`、`6707019520`、`11819728256`。

### 索引枚举样本

以下来自 `ffxiv/010000.win32.index`，序号从零开始。哈希为 directory/filename 组合键；
未恢复原始路径，不将材质路径当作模型路径。

| 编号 | Index 序号 / 哈希 | DAT / 绝对字节偏移 | SHA-256 |
| --- | --- | --- | --- |
| B0 | 113 / `0050d46a701019f3` | `010000.win32.dat0` / 31060736 | `d03d30dc521eb87e05ef359c87595e31e543b928b7e45a96d18fd49e493455fe` |
| B1 | 984 / `03755e2bca1ef3c6` | `010000.win32.dat0` / 89515904 | `d1911e9921162343eba9f36c5e2148fb10784dab3009af9f01d6d1e7738499a3` |
| B2 | 985 / `03755e2bca830414` | `010000.win32.dat0` / 1097222400 | `71d0b8ebbfe74b52d4e0aa14efceeeeefc3788ab5b4b0cb02be34ec0b64838d2` |
| B3 | 1167 / `040dee388cd70897` | `010000.win32.dat1` / 403897472 | `0272099d73ae3ea2a83dced49a5f02f2f6d2fd4ee19a926df32460a2e8861a84` |
| B4 | 1260 / `045908f2cdd8900b` | `010000.win32.dat1` / 59152640 | `752b2ff411212b827a96184ad8ca923f257798d81468083f99203e985242f17b` |

`tools/find_model_variants.py` 每包最多检查前 10000 条普通 index 记录，按 bgcommon、bg 顺序，
找到五份候选即停止。实测检查 215 份模型条目，在 bgcommon 记录 1260 处达到五份候选；
没有据此声称已遍历场景模型或其他分类不存在。

## 格式结论与独立观察

- **已观察**：136 字节 declaration、8 字节 element、首个 stream `0xff` 终止；
  60 字节 LOD、36 字节 mesh、16 字节 submesh；offset/stride 与 buffers 的范围吻合。
- **已观察**：类型 2、5、8、13、14、17。Python `struct` 用 binary32/binary16 解码浮点，
  对所有字段的所有顶点生成标量位模式；Rust 与 Python 的完整 fingerprints 一致。
  类型 0/1/3 的客户端映射与受控位模式见上述 binary32 验证，没有真实 MDL 样本覆盖。
- **已观察**：C2 的 12095 个顶点使用 byte8 权重/索引；相关 mesh 的 stream 字节为 `0x06`，
  实际只有两条流。全部非零权重的 palette-local 索引有效，所有 byte 权重和为 255。
  高位的完整游戏控制含义未证实。
- **已观察**：该版本 palette 为四字节 descriptor，索引采用 descriptor-relative、4 字节单位偏移。
  `+0x2c` 的 pool u16 总量包含对齐，palette 项均可解析到真实骨名。
  来源中固定 132 字节的旧声明与这些文件不吻合，未照搬为此版本布局。
- **已观察**：34009 条 shape-value 引用有效。C2 的 occurrence 5828 大于 mesh 顶点数 5580，
  但小于 index count，其 index 值为 650、replacement 为 3100。这排除了将 base 字段当作顶点号的解释。
  C3/C4 也有 occurrence 超过 vertex count 的例子。
- **已观察**：B1/B2/B4 有非空 water 分类；B0/B3 分别有 crest-change/material-change 范围。
  B1/B2 为两 LOD，其他样本为三 LOD。未执行 shader 或观察游戏 draw 调度。
- **推断且交叉验证**：model header `+0x24` 对应附加 32 字节包围盒数量；B1/B3/B4 为 7/12/9。
  忽略它们会误将附加盒当作整体盒，分别有 2776/15871/10439 个顶点越界。
  使用 `4 + bone_count + additional_count` 后，十份样本所有 position 均在整体盒内（容差 0.002），
  普通样本剩余前置区为 4/8/12/16 字节的 padding 编码。附加盒关联对象未知。
- **未知**：C2 的 runtime tail 含 203536 字节的扩展与 padding。
  header `+0x30` 为 12700，与总顶点数吻合，`+0x2b` 为 10；
  `12700 × 16 + 10 × 32 + 16 = 203536` 是长度关联，不足以赋予记录业务含义。
  库保留原始字节，未将其标为无用填充或伪造类型化语义。

## 复现与固定回归

安装只读，派生文件写入 `/tmp/opencode`。每个输出目录必须不存在。

```sh
nix develop -c cargo build --features local --example local_sqpack --example extract_entry --example validate_mdl
python tools/verify_model.py /home/azurice/Games/ffxiv/game/sqpack /tmp/opencode/model-evidence-01 chara/weapon/w2001/obj/body/b0102/model/w2001b0102.mdl chara/equipment/e0001/model/c0101e0001_top.mdl
python tools/verify_model.py /home/azurice/Games/ffxiv/game/sqpack /tmp/opencode/model-evidence-02 chara/human/c0101/obj/face/f0001/model/c0101f0001_fac.mdl chara/equipment/e0600/model/c0101e0600_top.mdl chara/human/c0101/obj/hair/h0001/model/c0101h0001_hir.mdl
python tools/find_model_variants.py /home/azurice/Games/ffxiv/game/sqpack /tmp/opencode/bg-model-discovery-01.json
python tools/verify_model.py /home/azurice/Games/ffxiv/game/sqpack /tmp/opencode/bg-model-evidence-01 @ffxiv/010000.win32.dat0:31060736 @ffxiv/010000.win32.dat0:89515904 @ffxiv/010000.win32.dat0:1097222400 @ffxiv/010000.win32.dat1:403897472 @ffxiv/010000.win32.dat1:59152640
python tools/verify_mdl.py /tmp/opencode/mdl-evidence-02 /tmp/opencode/model-evidence-01/0.mdl /tmp/opencode/model-evidence-01/1.mdl /tmp/opencode/model-evidence-02/0.mdl /tmp/opencode/model-evidence-02/1.mdl /tmp/opencode/model-evidence-02/2.mdl /tmp/opencode/bg-model-evidence-01/0.mdl /tmp/opencode/bg-model-evidence-01/1.mdl /tmp/opencode/bg-model-evidence-01/2.mdl /tmp/opencode/bg-model-evidence-01/3.mdl /tmp/opencode/bg-model-evidence-01/4.mdl
```

`verify_model.py` 对全部块使用受长度限制的 Python zlib 解压，与 Rust 的逻辑 MDL 逐字节比较。
`verify_mdl.py` 对名称、palette、submesh、包围盒、ElementId、shape 表和全部顶点/索引作独立观察。
FNV-1a scalar fingerprints 的输入是小端 u32 的浮点位模式或整数，覆盖所有 declaration element 和 index；
它用于对比，不替代样本身份 SHA-256。`tests/fixtures/mdl-observations.json` 固定十份样本、完整语义行的 SHA-256
及 mesh/vertex/index/shape 数量；验证器强制比较已登记样本，不将新样本自动记为固定回归通过。

总量：78 个 mesh/LOD view、61597 个顶点、135486 个 index、34009 条 shape-value。
十份 MDL 与 Python 的语义行及完整 scalar/index fingerprints 相等。
工具：Python 3.14.8、zlib 1.3.2；平台与安装版本同上。

## 验证边界

- glass 的非空真实范围缺少覆盖；glass 有独立 range 受控测试。其他四类的真实记录见下节。
- terrain-shadow 的八字节 half4 顶点有真实记录；其他 stride 的编码没有该证据。
- ElementId 的字节、父骨名及变换数值已验证；rotation 求值约定与挂点行为需骨架证据。
- 多 usage-index 访问保留原始编号；color1/normal1/flow 等非空声明需补充覆盖。
- 人脸 runtime 扩展业务语义未知；未用近似数据或空结果替代。
- 不覆盖其他 MDL version 或平台，不宣称穷尽支持。

`tests/mdl.rs` 另验证截断、4096 次确定性变异、非法引用、stride 范围、输出/工作预算、
byte8 蒙皮、glass range、shape occurrence/delta、扩展字节保留及附加盒。
这些受控用例不是游戏变体的样本替代品。

## Shadow、fog、light-shaft 与 terrain 的非空样本

`tools/find_model_roles.py` 对 base bgcommon/bg/chara 各限制 100000 条 index 记录、
1500 个模型，只独立解压 runtime 段，每份模型上限 16 MiB、runtime 上限 2 MiB。
三个包分别检查 10573/4226/12471 条 index，均达到 1500 模型上限，4500 份
runtime 没有观察失败；找到 Shadow、TerrainShadow、VerticalFog、LightShaft 与
ElementId rotation 见证，Glass 在这批有界样本内没有出现。
没有读取额外第三方实现，不从 index hash 恢复或猜测原始路径。

`tests/fixtures/mdl-role-observations.json` 固定六份资源的 index ordinal/hash、
DAT/offset、完整资源 SHA-256 和结果报告摘要。首四份分别含 light-shaft 1、
shadow 2、vertical-fog 1、terrain-shadow 3 个非空 mesh/LOD range。每份的
Rust type-3 逻辑重建与 Python zlib 全字节对照；随后所有标准 mesh 的全部声明、
scalar、索引、元数据与独立 MDL observer 比较，共 17481 顶点。

terrain 样本的 model header 声明三个 terrain mesh 和三个 terrain submesh，
普通 submesh 数为零；独立游标按 attribute→terrain mesh→普通 submesh→
terrain submesh→material 次序抵达正确字符串引用和尾部包围盒。
每 LOD 的 terrain range 指向各自 20 字节记录，三个记录均为 stride 8、12 顶点、
42 indices、1 submesh。每条顶点用 struct 的小端 binary16 解出四通道，
36 个 XYZ 全部能在相同 LOD 普通 mesh 的 position 声明中找到精确相同值，
第四通道均为零。这同时支持 half4 数据布局与 LOD 相对顶点 offset 单位。
126 个 u16 indices 均小于对应 12 顶点；index start 42 的字节位置是 84，
没有误当成 42 字节。所有 terrain/submesh 字段、36×4 浮点位、126 indices
与 Rust `terrain_shadow_geometry` 逐项相等。受控测试另检查 LOD/mesh/vertex
越界、错误 index、offset 溢出和不同 stride 不被默认当成 half4。

ElementId 的两份见证均保留全部字段：一份 rotation 仅有约 `7.45e-17` 的残差；
另一份 ID 5 的第三 rotation 分量为 `180.0`、第二分量为负零。库存筛选使用
`abs(component)>0.001` 区分非平凡数值与接近零残差；阈值是工具选样政策，
不是格式精度要求。180 的取值支持角度解释的猜测，但不证明单位、轴序或
宿主姿态合成方式，库不执行该变换。

shadow 样本有五个有效 position 超出存储整体包围盒加固定 `0.002` 的边界；
例如下界 `-10.934733390808105` 与 half 解码值 `-10.9375`。固定容差是原十份
样本的观察条件，不能推广为格式有效性要求。half 的量化间隔足以解释这里的
小差值，但“包围盒一定来自量化前数据”没有额外证据。原十样本的包含检查保留；
此批工具独立比较所有 scalar、完整包围盒和索引，固定额外的 outside 计数为
`0,5,0,0,0,0`，不丢弃越界点、不改写字段、不增大采样误差阈值。

```sh
nix develop -c cargo build --features local --example extract_entry \
  --example validate_mdl --example validate_mdl_roles
python tools/find_model_roles.py /path/to/game/sqpack /tmp/opencode/mdl-role-inventory.json
python tools/verify_mdl_roles.py /path/to/game/sqpack /tmp/opencode/mdl-roles-check
```

所有原始字节、全行输出、重建中间段与报告写到安装和仓库之外的新目录。
