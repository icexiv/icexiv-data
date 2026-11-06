# SGB 家具来源与验证

## 需求和许可审查

需求来自 xiv-companion commit `a38d3c9bfb67fb441d17a77b344f77c83058dca0` 的
`docs/render-book/model-routing.not`：家具经 SGB 发现 MDL 与关联 SGB，不要求
实现完整场景播放、行为或角色装配。没有阅读该项目的解析实现或本项目 legacy。

Lumina commit `8802d66a3cb3032be2fcf998bb10e864221bdb13`：先重新获取根 LICENSE，
确认为 WTFPL v2（2004 Sam Hocevar），再阅读：

- `src/Lumina/Data/Files/SgbFile.cs`、`Parsing/Scene/Scene.cs`、
  `Parsing/Layer/LayerGroup.cs` 全文；
- `Parsing/Layer/LayerCommon.cs` 的 Layer、InstanceObject、BGInstanceObject、
  SharedGroupInstanceObject 结构与其读取方法；
- `Extensions/BinaryReaderExtensions.cs` 的 ReadStringOffset/ReadStringOffsetData/
  ReadRawOffsetData 相关片段；`Parsing/StringOffset.cs` 全文；
- `Parsing/Common.cs` 的 Transformation（及工具展开紧随的 BoundingBox）声明与读法。

阅读包括解析方法，不宣称只见过声明；没有复制、翻译或机械移植它们，没有引入
Lumina 依赖或执行其解析器，也不宣称严格 clean-room。Rust 采用独立的有界
字节地址运算和预算，Python 用 struct 建立另一路径观察。

来源使用 u32 字符串偏移、i32 表项；两类分别处理。SceneChunk 的多 group 循环
不能作为可靠的多组指针规范：本次采用的是单组真实字节观察，并没有推广其
`i*4` 寻址代码为多组布局。没有将已见源码搬到另一目录后称为独立来源。

## 样本与结构事实

只读授权 Win32 安装 `2026.10.06.0000.0000`。九条 SGB 路径、文件 SHA-256、
全字段行 SHA-256，以及七条目标 MDL 的相应哈希，固定于
`tests/fixtures/sgb-observations.json`。原始文件、展开结果与完整报告在仓库之外。

样本包括 indoor general 0001/0010/1000/0210/0220/0221、outdoor general 0001，
以及真实 SharedGroup 指向的 hol/hana 两份文件。为寻找关联场景，
`tools/find_sgb_samples.py` 在 base bgcommon index 对 indoor/outdoor general
1..2000 候选作有界路径查找，检查了 217 个存在的资源后取得三个 SharedGroup
引用；不是把任意字符串后缀当作引用。未命中的 ID 与实际读取/解码错误区分，
不能把前者作为任意类型缺失的证明。

九文件均为 SGB1、一个 SCN1、声明固定头 72 字节、一个 group，各含一个 layer。
group 起点既出现 `0x54`，也出现 `0x214`（fun0010），可验证其偏移原点确为
SCN1 payload 的绝对 `0x14`，不是块头 `0x0c`。layer/instance 表项相对表起点，
名字及 BG/SharedGroup 路径相对各记录头。指向全局字符串区而非紧随记录的数据
已由实际地址核对。空 collision 路径指向一个实际 NUL，并非跳过失败解码。

九文件共 54 个实例，逐项核对类型、ID、名字、三组三通道变换（486 个 f32 位）
和 BG/SharedGroup/collision 字符串。保留 layer/instance 表顺序和每次路径出现。
fun0210 指向 hol，fun0220/fun0221 指向同一个 hana；hol 只有 marker，hana 实例
数量为零。fun0220/fun0221 分别使用 general0143/0144 的模型路径，证实不能
仅由 owner 文件编号生成依赖。

所有已输出的模型/关联场景/非空碰撞路径，均另查 index 确认可定位；关联 SGB
的图闭包全部包含于九文件内。七个唯一目标 MDL 再由既有 Rust decoder 和独立
`tools/verify_mdl.py` observer 逐字段、完整 scalar/index fingerprint 对照：
17 个 mesh、33728 顶点、59943 索引全部一致。这同时扩充家具静态几何的证据，
不声称已执行 SGB 变换、碰撞解码或验证渲染画面。

## General 家具候选与关联闭包

`tools/verify_sgb_inventory.py` 对同一 Win32 安装的 base bgcommon index 枚举
indoor/outdoor general ID 1–2000 的 4000 条候选路径，不从任意字符串猜测
资源名。其中 1930 条可定位，2070 条在该 index 中确认不存在；IO、synonym、
解压或结构错误单独失败，不计为不存在。这个范围不代表完整家具目录。

从根沿非空 SharedGroup 指针去重展开，得到 2088 份文件。每份均由 Python
有界 raw DEFLATE 和 Rust SqPack 分别重建、逐字节比较，再由 Python struct
和 Rust SGB 逐行比较字段。全部文件为一个 SCN1、72 字节固定头、一个 group、
一个 layer；没有发现可用于证明多 scene/group 的样本。15796 个实例的公共头、
名字、TRS 位和 BG/SharedGroup 路径完全一致。

共 4064 次模型引用、993 次非空 collision 引用、288 次 SharedGroup 引用，
均另经 index 定位确认存在。闭包保留路径出现次数与拥有者，不把同路径的不同
实例合并成一个变换。type 1/6 之外共见到 11 种实例类型；只比较公共字段，
没有把未解读负载当作无外部依赖。碰撞只验证可定位，不声明解码其几何。
七份目标 MDL 的全字段验证属于前述九文件实验，不推广为全部模型引用的验收。

`tests/fixtures/sgb-inventory-observations.json` 固定数量、类型分布、索引身份
和逐文件清单摘要。清单包含路径、dat 地址、index ordinal/hash、文件及输出
行摘要、layer/instance 数与依赖，按确定的候选及广度优先顺序编码为
`json.dumps(samples, sort_keys=True, separators=(',', ':'))` 后计算 SHA-256。
完整清单、资源字节与两侧行输出保存在外部报告目录。重复验证要求摘要和统计
逐项匹配，任何新 profile 或缺失引用使验证失败，不自动降低为通过。

## 受控实验与边界

回归使用非紧邻数据、乱序实例地址、两个 layer、负 i32 表项、空 layer、未引用
的诱饵路径、负零、未知实例类型、错误偏移/终止符、非有限变换和预算。
另做 8192 次确定性字节变异。负表项及双 layer 是受控 wire 实验，不冒充实际
游戏文件；负 u32 字符串偏移没有被当作有效变体。库对多 scene/group 等未经
验收的布局显式返回 unsupported profile，不猜测偏移布局。

公共头之外的 light/VFX/sound/marker 行为及其他附加数据保留原文件，但没有
解析其全部负载；unknown 实例不能充当所有外部依赖均不存在的证明。
模型/共享场景依赖发现与完整场景语义分别验收，不将未知数据默认为单位行为。

## 复现

```sh
nix develop -c cargo build --features local --example local_sqpack \
  --example validate_sgb --example validate_mdl
python tools/verify_sgb.py /path/to/game/sqpack /tmp/opencode/sgb-check
python tools/verify_sgb_inventory.py /path/to/game/sqpack /tmp/opencode/sgb-inventory-check
```

工具使用全新输出目录，独立重建 SGB 的 type-2 字节并与 Rust 提取结果比较，
再逐行核对所有结构/TRS/路径和目标 MDL 数据。所有安装访问均只读，不运行
游戏、服务器、GPU、宿主行为或其他目标程序。
