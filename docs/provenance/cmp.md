# CMP 来源与验证

## 许可审查与阅读范围

- Ktisis commit `774e61399df57108feea1f86dee54fc5bdb42877`，根 LICENSE 为 GPL-3.0，
  先审查许可再阅读 `Ktisis/GameData/Chara/CharaCmpReader.cs` 全文件。
  阅读暴露了颜色块选择和读取方法；其内容没有体型参数布局，未作为体型解析模板。
- NetStyx/ARealmRepopulated commit `4db37f405a2986ed17fba6872ac0a261fab51887`，
  根 `LICENSE.md` 为 AGPL-3.0，先审查许可，再阅读 `doc/research/model-scale.md`。
  只读研究文档，没有读取该项目实现。文档含静态分析报告与伪代码，提供八种族、
  五 body type、两 clan 的索引关系、56 字节记录和男性/女性高度字段位置。
  报告没有完整目标二进制身份，本库不把其运行时结论当作本次直接观察。
- TexTools/xivModdingFramework commit `25b3ae72099c12915eb68a574d1e2b40cee0d1c9`，
  先读取 GPL-3.0 `LICENSE.txt`，再阅读
  `xivModdingFramework/General/DataContainers/CharaMakeParameter.cs` 全文件。
  其 14 个数据字段提供尾长与胸围字段的公开命名；阅读也暴露了读写、列表分配及
  per-gender 辅助方法，记录此接触。没有读取 `General/CMP.cs` 或其他解析实现。

上述 copyleft 项目仅作审查后的字段/格式资料，没有引入依赖、执行其解析器、
复制、翻译或照搬其实现。Rust 使用固定 profile 的借用视图、原始浮点数组、独立
索引检查和类型化范围访问，不采用公开报告中的 customize 回退过程。
不读取 legacy，不宣称严格 clean-room；项目发布许可决定保持独立，`publish=false`。

许可筛选还检查了 xivdev/Penumbra（根 LICENSE API 返回 404）、
imchillin/Anamnesis（元数据 NOASSERTION）及 ackwell/ironworks（MIT，
Copyright 2022 Saxon Landers & contributors；树中没有 CMP）。未读它们的 CMP 源码。
NetStyx 研究文档和上述数据声明不是获授权再分发的游戏字节来源。

## 原始样本与结构观察

Win32 用户安装 `2026.10.06.0000.0000` 的 `chara/xls/charamake/human.cmp`，
186752 字节。SHA-256 和全语义行哈希固定在 `tests/fixtures/cmp-observations.json`。
先于字段资料阅读，Python 直接观察到 `0x2c800` 处从 RGBA 风格字节转为有限浮点，
末尾恰为 `80 × 56` 字节；每条记录的 14 个数呈重复端点模式。

`tools/verify_cmp.py` 从文件末尾推导 80 条参数的位置，顺序读出所有颜色、14 个
原始浮点位，再与 Rust 按 race/body/clan 查询的结果逐项对比。全部 45568 色、
1120 个浮点标量、560 对有序范围一致。44800 个颜色的 alpha 为 255，768 个
具有其他 alpha 值；不能用“全部不透明”的假设裁剪颜色表。

这项文件对照验证字段位置、数值范围、性别子区与索引表达；它自身不证明
slider 消费、NPC 特殊 body type 应用、胸围骨空间或颜色选择规则。
指定 executable 的字段与百分比消费静态证据见下文。
五 body type 的原始记录均能读取；特殊体型的游戏运行时含义保留为推断/未知。

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_cmp
target/debug/examples/local_sqpack /path/to/game/sqpack \
  chara/xls/charamake/human.cmp /tmp/opencode/human.cmp
python tools/verify_cmp.py /tmp/opencode/human.cmp /tmp/opencode/cmp-check
```

验证 Python 3.14.8。原始资源和全颜色/浮点输出位于仓库外。

## 原生静态分析环境

为 CMP/EQP 运行 `rea doctor --client opencode --json`，未运行游戏或附加进程。
已安装 REA 6.8.0，Node 24.21.0 低于该版本要求；OpenCode 无 REA 注册，
Ghidra/Hopper/IDA/Kuna 原生引擎均未配置。没有更改配置、安装引擎或更新 Node。
本页结论来自普通文件观察和许可审查后的资料，不称为 REA Evidence。
普通离线 objdump 的 EQP 消费点与字段证据见 [EQP](eqp.md)；它不提供 REA
原生引擎能力，也不建立 CMP slider/骨空间或 PBD 应用空间的完整证明。

## CMP 字段与百分比的原生静态证据

目标为获授权 Win64 安装的 `ffxiv_dx11.exe`，51910400 字节，SHA-256
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`。
安装版本文件为 `2026.10.06.0000.0000`；首选 image base `0x140000000`。
以下为 GNU objdump 2.46 的离线指令观察，地址为首选 VA，不是运行中地址；
没有启动、附加、模拟目标，不是 REA Evidence，没有改变工具环境或安装。

- `140426893..1404268c5`：以 `chara/xls/charaMake/human.cmp` 请求资源，
  结果保存在 utility `+0x240`。utility 身份与 EQP/PBD 记录使用同一固定程序。
- `14046d5c0..14046d6a7`：对有效 tribe ID 减一，商/余位分别形成 race/clan
  坐标，body type 形成两记录组。男性的基址是 `file+0x2c800`，女性是
  `file+0x2c810`；记录步长 56。取相邻两个 f32，按百分比除以常量 100 后
  对差值缩放，再加首端点。输入 `255` 另返回 `1.0`；`101–254` 使用 0。
- `14046d490..14046d570`：接收分开的 race/clan、sex/body 参数，男性基址
  索引 `0xcb7 × 56 = 0x2c808`，女性基址 `0x2c818`；同样取相邻端点插值。
  超过 100 的百分比使用 0，没有套用身高的 255 特例。
- `14046d180..14046d2bd`：男性/默认分支复制 `(1,1,1)`，女性分支按同一
  56 字节网格取得 `0x2c820/824/828` 与 `0x2c82c/830/834`，分别执行
  三分量端点插值。超过 100 的百分比使用 0。
- `140445fa0..1404460b2`：从角色状态字段准备输入，调用三轴和尾长助手，
  结果写入该调用路径引用对象的 `+0x68..+0x70` 和 `+0x74`。这只定位参数
  消费的一个桥接点，没有追踪到全部骨名、骨空间及动画姿态合成。

这些控制流独立确认了公开资料的字段布局、有效坐标映射及正常百分比插值
含义。原生 body/race 回退和特殊输入策略只作为观察事实登记，没有把其树、
条件、回退、对象偏移或汇编过程复制、翻译或机械移植到库。
`verify_client_cmp.py` 校验程序身份、五段区间摘要、二十个锚点、路径、
调用目标和三个除法常量引用；固定记录为
`tests/fixtures/client-cmp-observations.json`，反汇编只在外部目录保存。

## 独立范围数学验证

`Range::at_percent`/`VectorRange::at_percent` 是按加权平均定义自行实现的
数学接口，采用 `(min*(100-p)+max*p)/100` 的 f64 中间值后舍入到 f32，
避免有限 f32 极值相减溢出，并减少中间舍入。0/100 直接返回文件端点位。
它们拒绝大于 100 的输入，不替调用者选择 customize 哨兵、回退或男性胸围
单位向量；也不声称复现原生 SSE 的每一步舍入。原始端点访问不变。

`verify_cmp_interpolation.py` 通过 Python struct 独立读出 80 条实际记录，
用 Fraction 对端点的精确二进制有理值计算全部百分比。560 个标量范围各
101 个结果，共 56560 个 f32 位与 Rust 完全一致。示例还核对每条记录的
155 个非法百分比组明确返回 `InvalidPercentage`，不误作客户端 255 特例。
样本与结果摘要固定在 `tests/fixtures/cmp-interpolation-observations.json`。

受控回归包括负端点、三轴向量、0/25/50/100、所有非法 u8 百分比、带符号零
的端点保真、±f32::MAX 间插值、NaN/无穷/倒序端点。没有外部解析依赖或
附加分配。此验证是数学求值与字段语义证据，不替代骨空间和 NPC 分支证据。

```sh
nix develop -c cargo build --example validate_cmp_interpolation
python tools/verify_client_cmp.py /path/to/game/ffxiv_dx11.exe /tmp/opencode/client-cmp-check
python tools/verify_cmp_interpolation.py /tmp/opencode/human.cmp /tmp/opencode/cmp-math-check
```
