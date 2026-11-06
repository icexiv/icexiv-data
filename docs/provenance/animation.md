# PAP 动画样本观察

## 来源和边界

只读用户授权的 Win32 安装 `2026.10.06.0000.0000`，资源
`chara/human/c0101/animation/a0001/bt_common/resident/idle.pap`，33541 字节。
原始文件 SHA-256 和观察锚点保存在 `tests/fixtures/pap-observations.json`。
wire 资料和许可审查沿用 [Havok 记录](skeleton.md)。没有运行游戏或 Havok runtime。

另审查 PredatorCZ/HavokLib commit `ef5d5c6f5ab8d7f32cb4e8983300b8db61e4dc6b`：
先获取根 LICENSE 的 GPL-3.0 文本，再阅读 `source/hka_spline_decompressor.hpp`、
`include/internal/hka_splineanimation.hpp`、`classgen/hka_animation_spline.py` 与
完整 `source/hka_spline_decompressor.cpp`。文件标注 Copyright 2016–2023 Lukas Cone、
GPL-3.0-or-later。阅读涉及 wire、量化和样条算法；未复制、翻译或机械移植实现，
没有引入该库。其未知旋转编码返回 identity 及忽略 float 轨道的行为不采用。
SoulsAssetPipeline commit `d11caf989c917c7a43e2c7559915b1c5af218153` 仅列出
spline 路径，未阅读其 spline 源码。这里不宣称严格 clean-room。

## 容器和对象观察

原始前缀为 `pap `，其后 u32 为 `0x00020001`，`+8` u16 为 2。
26 字节头之后是两个 40 字节记录，名字为 `cbna_add_dmg_f`、`cbnm_id0`。
记录的 `+34` u16 分别为 0、1。记录内另有 `+32` 的 15/0 值和其他零字节，
其完整语义不由本次取值赋名。

`+18` u32 为 106，`+22` u32 为 33184；两位置之间的 33078 字节为完整 Havok
binary tagfile v3。`tools/observe_tagfile.py` 独立解析到 end，得到 32 类、6 个
remembered 对象。animation container 的 animations 为对象 `[6,4]`，bindings 为
`[5,3]`；binding 5 指向 animation 6，binding 3 指向 animation 4。
因此不能用对象实际出现顺序代替 container 数组顺序。

两动画均为 `hkaSplineCompressedAnimation`，声明 type 为 3：

| 对象 ID | duration 秒 | transform tracks | frames | blocks | max frames/block | mask/quantization size | data bytes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 4 | 约 2.3333333 | 97 | 71 | 1 | 256 | 388 | 4880 |
| 6 | 约 0.96666664 | 95 | 30 | 2 | 18 | 380 | 25680 |

binding 的 transformTrackToBoneIndices 长度分别为 97 和 95，原始骨架名为
`n_root`；binding 5 还声明 blendHint 为 1。完整行为含义和 spline 曲线求值
不由这些元数据或名字推定。

将同一真实 Havok 字节段包入一个自行生成的 1300 SKLB 头，交给 Rust 通用
tagfile reader，可完整读取为 32 类、6 对象、0 骨架。这是受控 wrapper 实验，
不是游戏生成的 SKLB，也不是 PAP/spline 解码完成证据。

## 重现

```sh
target/debug/examples/local_sqpack /path/to/game/sqpack \
  chara/human/c0101/animation/a0001/bt_common/resident/idle.pap /tmp/opencode/c0101-idle.pap
python tools/observe_tagfile.py /tmp/opencode/c0101-idle.pap
```

## 曲线和采样证据

`tools/observe_spline.py` 使用独立 Python 字节游标读取每条轨道，保留量化解码后
的控制点。三个块的 transform 结束位置分别为块内 4872、15332、10328，
与声明的 floatBlockOffsets 全部相等，剩余块尾为零填充。实际量化覆盖 u8/u16
向量和 polar32/three-component40/three-component48，degree 1/2/3、静态、identity、
动态 XYZ、混合旋转掩码均出现。uncompressed quaternion 用受控编码实验验证，
不冒充游戏生成样本。

Rust 使用局部 de Boor 控制多边形算法，Python 使用全 knot 向量的 Cox–de Boor
基函数递推，两者不同的数值求值路径对每块、每轨道的十个坐标
`0, .125, .5, 1, 3.25, 8.5, 12, 17, 29, 70` 逐分量比较。
块内共 28700 个 TRS 分量；再以每动画九个毫秒坐标
`-1, 0, 125, 500, 566.6666, 566.6667, 700, 1000, 2500` 验证时间转换、
端点钳制和帧 17 两侧的块选择。两部分合计 45980 个分量，绝对误差小于
`2e-6`（实测最大约 `5.93e-8`）。完整 bone 映射、原始骨架名、blendHint 和
PAP 记录名字也逐项比较；结果行哈希固定于样本清单。
双块动画的前块帧 17 与后块帧 0 全轨最大分量差约 `0.00138242`，
与独立量化造成的近连续性一致。此观察支持一帧重叠的块结构，不能当成
游戏运行时播放或最终姿态的逐位 oracle。

受控回归另验证单位/轴向 quaternion 编码锚点、二次曲线解析值、非有限时间、
端点钳制、预算、截断和 4096 次变异。该 idle 样本不能单独证明所有 PAP 压缩类型、
float 轨道或运行时 additive/blendHint 策略。

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_animation
python tools/verify_animation.py /path/to/game/sqpack /tmp/opencode/pap-check
```

独立工具逐项比较全部输出，记录版本、来源哈希、行哈希、最大误差和重叠帧差；
提取资源与完整报告写到安装和仓库之外的新目录。

## 索引定位的扩展包与容器对齐

同一 Win32 安装的 base chara index 中，`tools/find_pap_samples.py` 有界读取
最多 100000 条 index 记录、200 个 PAP，按动画轨道数、帧数、块数和编码组合
保留最多八种 profile。不从 hash 猜出原始路径，不以记录名字推断动作或物种。
八份定位信息、index ordinal/hash、资源长度与 SHA-256 保存在
`tests/fixtures/pap-variant-observations.json`。

一轮对 283 条 index / 200 个 PAP 的独立观察遇到 199 个观察失败，多数为对象流
尾部长度差异，另有一个零记录空包；这些失败不是资源损坏证据。逐字节检查发现，
PAP Havok 段可在 end tag 后
有零填充，timeline offset 是绝对文件偏移的 16 字节边界。按容器层验证该区域
后，41 条 index 中的 12 个 PAP 给出八种不同 profile，观察过程没有失败。
八份保留文件的实际填充长度依次为 `7,4,14,14,9,1,4,12`，且 timeline offset
均为 16 的倍数。原 idle 包则没有这段填充。它们证明存在两类段边界，不证明
独立 tagfile 或 SKLB 都允许任意尾随零字节。

受控回归对全部 16 种绝对偏移余数验证 PAP 填充，拒绝非零字节、额外整块填充
和有填充却不对齐的时间线；独立 `Tagfile::parse` 仍拒绝额外尾随零字节。
Rust 内部 prefix reader 只为容器报告 end tag 的消费长度，PAP 自行校验剩余区域。

扩展包合计 12 个动画、52 个块，轨道规模 92 或 169，帧数 26–171，单动画
1–11 块；存在 degree 1/2/3、旋转编码 0/1/2、向量编码 0/1。完整 binding、
名字与映射，以及每块全部轨道的十个坐标、每动画九个毫秒坐标，与独立
Cox–de Boor oracle 对照共 988170 个 TRS 分量，最大绝对误差
`4.737592256276457e-7`，低于同一 `2e-6` 比较界限。连同 idle 为九包、十四动画、
五十五块、1034150 个分量。没有额外第三方资料或实现咨询。

八包的相邻块重叠帧最大逐分量差为 0–约 0.0126366，最大值来自平移分量，
不是四元数符号翻转。idle 的约 0.00138242 不能成为所有文件的连续性阈值。
扩展清单逐包固定实际重叠差，采样输出仍逐项执行 `2e-6` 独立 oracle 比较；
这些边界差只作为压缩块观察，不是播放端误差许可或世界姿态证明。

复验同时验证 index 记录、Rust type-2 提取与独立 zlib 全字节一致、PAP 头、
end tag 消费位置、填充、全对象数量、container 数组、binding、所有采样值和
固定输出哈希。全部提取文件与输出按样本分目录保存：

```sh
nix develop -c cargo build --features local --example local_sqpack \
  --example extract_entry --example validate_animation
python tools/find_pap_samples.py /path/to/game/sqpack /tmp/opencode/pap-inventory
python tools/verify_animation.py /path/to/game/sqpack /tmp/opencode/pap-variants-check \
  tests/fixtures/pap-variant-observations.json
```

这八包都没有 float tracks，旋转编码 5 仍是受控编码证据。其他压缩类型、
其余量化编码与运行时 blendHint 组合不由这批证据覆盖。

## 零记录空包

index ordinal 79、hash `000a40e56259446d` 对应 dat0 offset `8369887616` 的
32 字节 PAP 有 count=0、records offset=Havok offset=26、timeline offset=32。
Havok 段只有六个零字节，没有 tagfile magic、对象或 end tag；timeline 为空。
`tests/fixtures/pap-empty-observations.json` 固定完整资源和输出哈希。
独立 struct/index/zlib 观察与 Rust 提取逐字节相等，Rust 返回 `tagfile=None`、
六字节 `havok_padding`，不创建默认动画或伪造空 Havok 对象图。

```sh
python tools/verify_animation.py /path/to/game/sqpack /tmp/opencode/pap-empty-check \
  tests/fixtures/pap-empty-observations.json
```

## 有界编码库存与大数组、多块包

`tools/inventory_animation.py` 只读 base chara 的前 100000 条 index，最多选
2000 个 PAP，每份上限 4 MiB。顺序选择检查 5793 条 index、211 个目录 hash，
得到 2000 包、12 个零记录包与 3602 个 container animation 引用；按每目录
最多 16 包分层选择检查 16986 条 index、606 个目录 hash，得到 2000 包、
43 个零记录包与 3766 个 animation 引用。两轮样本有重叠，不合计为 4000 个
互异包。全部非空引用的类型都是 hkaSplineCompressedAnimation；观察到的旋转
编码仅 0/1/2，没有 float tracks、零 transform tracks 或其他压缩类。这只是
两轮有界库存的结果，不是安装中全局不存在这些变体的证明。

库存的 struct/zlib 观察限制 first block 为 65535 字节、type-2 表最多 16384 项，
完整 PAP 最多 4 MiB。Python 对象观察明确使用 4 Mi 元素数组与 8 Mi value
预算；该预算不改变 Rust 默认值。两轮报告 SHA-256 和下述资源身份固定于
`tests/fixtures/pap-large-observations.json`，工具复验时强制比较。

两轮均命中 index ordinal 5525、hash `03fe5207c9e63460`、dat0 offset
`12471626112` 的 1263624 字节包。动画对象 4、binding 3；210 transform tracks、
1206 frames、173 blocks、每块最多八帧，步长七帧。data 字节数组长 1259776，
其中没有 float tracks。默认 Rust array-length 预算 1000000 确切返回
`BudgetExceeded("array length")`；显式设置 2000000 后完整解析与解码成功。
没有将资源限制改写为格式规则，也没有隐式提高全库默认预算。PAP 头、十字节
Havok 对齐、全部 binding/name/骨映射及完整 type-2 字节均独立比较。

采样逐块覆盖全部 210 轨道和十个局部坐标；毫秒采样另包含九个基本坐标、
172 个内部块边界的前/中/后三点，以及 duration 前、端点与 clamp 后，
共 528 个毫秒坐标、110880 个 time/track 样本。总计 4741800 个 TRS 分量，
局部 de Boor 与独立 Cox–de Boor 最大绝对误差 `9.411119421542935e-7`，
仍使用 `2e-6` 比较界限。各边界的块选择、末块和 duration 钳制也逐项相等。
重叠帧最大原始分量差约 `0.0096721983`，按样本固定，不将其当成曲线求值容差。
连同九个较小包，合计十个非空包、十五动画、228 块、5775950 个独立对照分量。

数组预算回归另用 48 字节 payload 明确检查预算 47 拒绝、48 成功；大包复验
会先强制验证默认预算失败，再以显式预算生成固定哈希的全部输出。

```sh
nix develop -c cargo build --features local --example extract_entry --example validate_animation
python tools/inventory_animation.py /path/to/game/sqpack /tmp/opencode/pap-encoding-inventory
python tools/inventory_animation.py /path/to/game/sqpack /tmp/opencode/pap-stratified-inventory --stratified
python tools/verify_animation.py /path/to/game/sqpack /tmp/opencode/pap-large-check \
  tests/fixtures/pap-large-observations.json
```
