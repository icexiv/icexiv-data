# EQP 来源与验证

## 来源审查

公开字段资料来自 TexTools/xivModdingFramework commit
`25b3ae72099c12915eb68a574d1e2b40cee0d1c9`，先审查 GPL-3.0 LICENSE.txt，
再阅读 `xivModdingFramework/Models/DataContainers/EquipmentParameter.cs` 全文件。
该文件暴露了槽位字节区域、位命名、BitArray 访问和修改、GimmickParameter、
EquipmentDeformationParameter 的两位字段等。没有读取 `Models/FileTypes/Eqp.cs`。
许可审查范围与限制同 [CMP](cmp.md)。没有复制、翻译或照搬解析代码、枚举实现或
BitArray 访问结构；位位置作为公开数据格式事实，与真实数据逐位对照。
未增加该项目或其他 FFXIV 解析库依赖，不读取 legacy，不宣称严格 clean-room。

## 样本与独立观察

只读 Win32 安装 `2026.10.06.0000.0000` 的
`chara/xls/equipmentparameter/equipmentparameter.eqp`，23040 字节。
SHA-256 固定在 `tests/fixtures/eqp-observations.json`。
首 u64 为 `0x638400e48000007f`，18 个置位，与文件长度 `18 × 160 × 8` 精确一致。
块 0–6 之后跨过缺省区，再出现 31、34、37 等逻辑块。

`tools/verify_eqp.py` 顺序展开存储块，与 Rust 的 popcount/rank 随机查询逐项
对比所有 10239 个非保留位置：2879 个有存储值、7360 个没有对应块。
原始 u64 和五个槽位区域全部相同；报告保存每一位的真实置位数量。
这些文件观察支持 bitmap、块宽、位顺序和槽位切分，不独立证明每个公开位名
的行为。跨槽区域来源与部分 attribute 关联的静态证据见下文；耳/尾/头发的
完整条件、NPC 差异和未跟踪消费分支仍有缺口，不作为完整显隐策略验收。

受控回归覆盖最高逻辑块 63、块空洞、保留 ID 0、精确尺寸、全部 64 个单置位、
原始区域重组及位查询边界。缺省块返回 None，不假造游戏的默认掩码。

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_eqp
target/debug/examples/local_sqpack /path/to/game/sqpack \
  chara/xls/equipmentparameter/equipmentparameter.eqp /tmp/opencode/equipmentparameter.eqp
python tools/verify_eqp.py /tmp/opencode/equipmentparameter.eqp /tmp/opencode/eqp-check
```

验证 Python 3.14.8。原始资源与展开记录不入库。

## 客户端离线静态证据

目标是用户授权只读安装中的 `game/ffxiv_dx11.exe`，51910400 字节，SHA-256
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`。
安装版本文件为 `2026.10.06.0000.0000`；该文本不代替 executable 身份校验。
PE 是 x86-64，首选 image base `0x140000000`。下列 VA 均是 image base 加 RVA，
不是运行中地址，不涉及 ASLR 或进程读取。

REA 的原生后端状态沿用 [环境记录](cmp.md)：没有已连接的 REA 工具和配置的
原生引擎。本次使用已安装 GNU objdump 2.46 与独立 PE/字节观察，没有安装引擎、
修改配置、启动/附加/模拟游戏或调用目标代码。报告是普通离线静态观察，
不称为 REA Evidence。

`observe_client_paths.py` 有界读取 PE 的 section/pdata 和路径/attribute 字符串，
扫描 RIP-relative LEA 候选。候选再由 objdump 的指令边界与相邻控制流确认。
程序文件及完整反汇编都在安装或外部输出目录，不复制进仓库。

| 静态位置 | 观察 |
| --- | --- |
| `140426558` / `140426576` | EQP 路径用于资源请求，结果保存为 utility 对象的 `+8` 指针 |
| `1400d0faf` / `1400d0fc2` | utility 指针保存于首选 VA `1428fd980`，随后进入资源初始化 |
| `14042bae0..14042bc7d` | 读取 utility `+8`，查询各 set 并选择五个参数区域，处理两个 coverage selector |
| `14046d2e0..14046d419` | EQP 的 160-word 块/bitmap/rank 查询；消费者把请求 ID 0 改用 ID 1 |
| `140447440..14044759c` | 解析的八字节声明存于栈上，同一指针直接传给 attribute 消费过程 |
| `140447b70..140447f1b` | 启用/比较条件、coverage 分支、attribute 清位与 shape 置位 |
| `1402744b0..1402744f3` | 以名称查模型 map，命中返回节点 `+0x28` 的 u16，缺失返回 ffff |
| `140322b7e..140322be0` | runtime 字符串区长度后，model header `+6` 读为 attribute count |
| `1403231b4..1403233d0` | 从零开始的表下标与属性名字关联，存入 map 节点 `+0x28` |

阅读范围还包含资源 handle 构造/析构和 vtable 邻域、utility 调用点以及上述流程的
前后文；`client-utility-01.asm` 与 `client-model-map-02.asm` 分别保留有界 utility/
model 反汇编，仅查询相关片段。没有把客户端 bitmap/rank、树查找、参数合并、
显隐或 shape 驱动算法复制、翻译或机械移植进库。Rust 保留既有独立解析器，
只增加文件字段视图与受工作预算限制的借用字符串表线性查询，不宣称 clean-room。
没有额外第三方解析实现或依赖；程序为获授权本地专有产物，不作再分发许可推定。

### 已确认的字段关系

- Body word 中 bit 10/9/8 分别选择该记录的 head/hand/leg 区域；来源不满足
  时读取对应槽自己的 set。所选 leg 的 bit 5 再决定 foot 区域来源。
- Hand 与 Foot byte 的 bits 1–2 是耦合两位 selector；它们与 Body bits 3–4、
  Leg bits 2–3 以及各自 byte 的 bit 3 一起参与比较。独立 Boolean 位名不能
  替代组合字段解释，API 同时保留原始字节与 selector 数值。
- 所跟踪的 arm/leg 分支按启用与比较条件分别引用 `atr_hij/atr_ude`、
  `atr_sne/atr_hiz` 和 `shp_hij/ude/kat`、`shp_sne/hiz/mom`。neck 分支引用
  `atr_nek/shp_nek`，还检查 head enabled、neck/gorget、帽子声明和角色状态。
- 命中 attribute 名称后，u16 文件表下标用于生成模型掩码中的一个位；没有该
  名称时不修改对应位。shape 的查询目标 map 为另一个区域，不能与 attribute
  表混成一个全局索引空间。

这些是指定 executable 中的静态控制流，不是一次运行观测；未据此提供完整
角色装配或 hidden-mesh 求值器。consumer 的 ID 0→1 行为也不改变文件中位置
0 是 bitmap 的事实，`Eqp::entry(0)` 仍是错误，缺省块仍显式为 None。

### 独立资源与 API 对照

`verify_eqp_semantics.py` 用顺序块展开对照全部 2879 个存储记录和 7360 个缺省
记录；十个字段共 28790 个值逐项相等。Hand selector 0/1/2/3 的记录数为
1867/471/325/216，Foot 为 1822/418/345/294，实际表覆盖所有四个编码。

另外对 e0001/e0600 的 top/dwn 四份真实 MDL 作独立 type-3 全字节重建、完整
几何/元数据比较和名称→位查询，共 11269 顶点、12 条 attribute 名记录。
`atr_ude` 在两个 top 分别对应 bit 2 和 bit 0；`atr_hiz` 在两个 dwn 分别对应
bit 0 和 bit 1。所有原始 submesh mask 都独立比较，不依据名字硬编码全局位。
不存在的查询返回 None；预算、重复名字和超出 u32 表达范围的下标有受控回归。

exe/指令区段摘要与字段/字符串锚点固定在
`tests/fixtures/client-eqp-observations.json`，资源身份和全结果摘要固定在
`tests/fixtures/eqp-semantic-observations.json`。复验强制检查固定摘要，不从
模糊字节匹配自动接受另一版本的程序。

```sh
nix develop -c cargo build --features local --example local_sqpack \
  --example validate_eqp_fields --example validate_mdl --example validate_mdl_attributes
python tools/verify_client_eqp.py /path/to/game/ffxiv_dx11.exe /tmp/opencode/client-eqp-check
python tools/verify_eqp_semantics.py /path/to/game/sqpack /tmp/opencode/eqp-semantics-check
```

耳/尾/头发的全部条件、NPC/特殊角色分支、覆盖之外的 shape/morph 消费，以及
PBD 矩阵应用空间没有由这些局部流程建立完整证据。

## Head hair 三位字段与耳部关联

同一固定 executable 的 `1404489a0..140448e54` 是另一条模型属性消费过程。
入口将第二参数保存到 rbp；`140448a2a` 从 `[rbp+5]` 读取 EQP head 的低
字节，先检查 bit 0 enabled，`140448a65..140448a6f` 右移一位、取低三位、
减一后检查 0–5。跳转表位于 RVA `0x448e54`，六个目标依次为
`0x448a8b,0x448aee,0x448b4d,0x448aee,0x448b5a,0x448bc9`。
因而整字 bits 41–43 是一个共同进入分支选择的三位值，不能只以公开的
三个 Boolean 位名说明其消费。code 0/7 越过这张表；code 2/4 共用入口。
没有将各跳转分支的显隐操作移植到 Rust。
调用桥 `140447902..14044790f` 取同一栈上 EQP 区域 `[rsp+0x68]` 作为第二
参数并直接调用此过程；区间摘要与 call 目标也在复验中固定。

分支中出现 `atr_kam`、`atr_sta`、`atr_bak` 和 `shp_hib` 名称查找。部分
分支还取分类助手 `140449f90..140449ffc` 的返回值，并检查对象 `+0xa20`、
`+0xc1c` 等状态和 head 的帽子位。助手按一个对象字段进行重分类，其所有
入口、上游状态来源和最终策略没有在这项局部观察中验收，不从中文耳部名称
猜测额外分支。

`140448c51/140448c62/140448c77` 分别测试所选 EQP 字节 6 的 masks
`0x04/0x08/0x10`，即整字 bits 50/51/52，在分类条件下关联 `atr_mim`、
`atr_top`、`atr_hrn`。这些条件另受 hair 分支和其他状态影响，只作为这些
原始字段与模型名称之间的局部证据，不构成完整耳部或 NPC 显隐策略。

`verify_client_eqp.py` 复验两个消费/分类区间摘要、三处字段锚点、七个字符串
引用及实际跳转表；身份和边界并入 `client-eqp-observations.json`。证据仍是
GNU objdump 2.46 离线观察，不是执行、仿真、运行快照或 REA Evidence。
没有读取额外第三方解析实现、引入依赖或接触归档源码。

`HeadFlags::hair_coverage_code()` 暴露原始三位数值；已有单个位 accessor
保留为 raw bit view，不按优先级组合出结果。`verify_eqp_hair.py` 对实际 EQP
顺序展开全部存储块，独立取得 head 整数并用除法/余数取得 code，对照 Rust
随机 ID 查询，保留 7360 个缺省位置为不存在。2879 个存储条目的 code
0–7 数量依次为 `[2113,452,132,123,16,7,36,0]`。另对全部 256 个低字节、
含非零其余 head 位的受控值核对 code 与三个位 getter，八种 code 都保留。
这项数学对照不执行客户端 switch，不把 code 7 没有实样本当成格式禁止。

`tests/fixtures/eqp-hair-observations.json` 固定实际表身份、完整行摘要和报告
SHA-256；实际字节、两侧输出和反汇编保存在外部目录。资源联跑复算该字段
对照，并继续运行既有 cross-slot/coverage/MDL attribute 验证。

```sh
nix develop -c cargo build --example validate_eqp_hair
python tools/verify_eqp_hair.py /tmp/opencode/equipmentparameter.eqp \
  /tmp/opencode/eqp-hair-check
```

## 耳与尾的槽位、启用条件及实文件关联

### 静态读取与边界

目标和离线工具身份沿用本页，没有读取额外第三方实现，没有运行、附加或
模拟游戏。读取 `140447440..140447b70`、`1404489a0..140448e54`、
`140448e80..140449120`、`140449640..140449a10`、
`140449f90..14044a000` 和 `140443420..140443710`，并只读检索本地程序
中的 `tail/zear` 路径字符串。原始程序与反汇编仍留在外部目录。
`140449640` 的额外 EQDP/属性消费不是本项移植或验收的对象。

以下为指定分支的指令/字段观察，不执行它们，也不构造对应角色状态机：

- `140443612..140443676` 比较模型槽参数 12，在对象 `+0xa20` 字节等于 8
  时使用 `chara/human/c%04d/obj/zear/z%04d/model/c%04dz%04d_zer.mdl`，
  对应其他分支使用 `tail/t%04d/..._til.mdl`。主显示消费点从 `+0xa8`
  指向的模型数组偏移 `+0x60` 读取该槽（12 个指针的偏移）。没有据此
  移植路径生成、NPC 编号修正或装配。
- `14044799a..1404479bd` 的普通分支分别以 Body enabled 和 Leg enabled
  为门控，检查整字 bit 13、22。任一启用区域的 tail 位为零都能清除模型
  `+0x28` 的低位；另一区域的置位不会把它重新置一。检查对象是此前
  `14042bae0` 解析到栈上的区域，不保证五个区域来自同一个装备 set。
- `14044794d..140447996` 是类型 8 分支：Head 位 57 与 Head enabled
  必须置位，角色 `+0xc1c` 低位必须为零，才施加这里的限制。bit 59 为零
  时用 bit 53 决定是否清除模型启用位；bit 59 为一时改看角色 `+0x90`
  的 bit 31，后者置位会清除模型启用位。未据此给 bit 59 或角色状态
  赋予外观业务名字。这只是一个静态消费点，其他显示覆盖仍未合并验收。
- `140449f90` 按 `+0xa20` 字节选择分类结果。跳转表七项对应输入 2–8，
  返回值依次为 `[1,1,2,0,3,5,4]`；其他输入走零结果。固定这些数值和
  返回块，不将分类结果直接命名成种族 enum，亦不把它作为独立显示规则。
- 第二条脸模型路径在 `140448fdb..140449143` 读取同一个 Head 字节。
  其 `1404490c1/1404490d2` 检查 bits 50/51，并分别查询 `atr_mim`、
  `atr_top`；前文的头发模型路径也有对应名字，但它们的目标模型不是同一
  指针。分类、hair code 与帽子条件使单个位名不足以说明最终显隐。

关键范围、13 个字段/槽位锚点、两个路径格式字符串和两个脸部 attribute
引用并入 `client-eqp-observations.json`。静态复验共 17 段；另核对原有
hair 六项表、分类七项表及五个立即数返回块，不在 Rust 内实现该分类算法。
`verify_client_eqp.py` 报告保留精确区段摘要、名字目标和锚点。

### 独立资源验证

授权 Win32 安装 `2026.10.06.0000.0000` 的四份完整 type-3 重建输出为：

| 资源路径（`chara/human/` 下） | MDL 字节数 | attribute 名 |
| --- | --- | --- |
| `c0801/obj/tail/t0001/model/c0801t0001_til.mdl` | 48260 | 空 |
| `c1801/obj/zear/z0001/model/c1801z0001_zer.mdl` | 30180 | 空 |
| `c0801/obj/face/f0001/model/c0801f0001_fac.mdl` | 889516 | 9 项，含 `atr_mim/atr_top` |
| `c1401/obj/face/f0001/model/c1401f0001_fac.mdl` | 916256 | 8 项，含 `atr_hrn` |

`verify_eqp_semantics.py` 的可选 fixture 选择这些实际文件，独立检查每个
压缩段、完整 MDL 字节、全部几何/元数据行及 submesh masks，共 24107 个
顶点。对 Python 从 MDL 名称表得到的局部下标与 Rust `attribute_bit`
逐项比较：Miqo'te 脸的 `atr_top/atr_mim` 是下标 3/8，Au Ra 脸的
`atr_hrn` 是下标 4；tail/zear 的空表不会被虚构出这些名字。
新增六个可选名字查询与既有九个查询合计 60 次，包括实际命中和确认无匹配。
资源路径和静态槽位分支的关联不证明这些文件在一次运行中的实际选择。

同一 fixture 对实际 EQP 的全部 2879 个存储记录另比较 13 个字段，共
37427 个值：Body/Leg enabled 与 tail、Head enabled、hair code、四个 ear
位、两个 hat 位及原始 bit 59。Python 顺序展开块，按整数除法/余数提取
期望值；Rust 使用公开 getter 和原始 bit 查询。7360 个缺省位置仍返回
不存在。既有十个字段的对照也继续执行；没有调用 native 条件树作为 oracle。

`tests/eqp.rs` 对上述具名 Boolean getter 检查全部 64 个单置位及其反码，
保证 enabled、其他区域和高位不会被 getter 自动组合为显示策略。
`src/eqp.rs` 的 API 文档明确记录共享 tail/zear 槽与门控边界。

`eqp-ear-tail-observations.json` 固定四个资源 SHA-256 及报告摘要
`e2d00a9148229a7a36863f81f14725b8c5fe3c3bc586925bb93edc60f9d8e44b`；
报告包含所有几何、attribute 查询与字段行的摘要。实际字节和完整输出留在
外部目录。此证据解释普通 tail 声明的门控并扩充耳部实文件关联，不宣称
所有耳/尾/头发分支、NPC 状态及最终显示覆盖均已验收。

```sh
nix develop -c cargo build --features local --examples
python tools/verify_eqp_semantics.py /path/to/game/sqpack \
  /tmp/opencode/eqp-ear-tail tests/fixtures/eqp-ear-tail-observations.json
python tools/verify_client_eqp.py /path/to/ffxiv_dx11.exe /tmp/opencode/eqp-ear-tail-static
```
