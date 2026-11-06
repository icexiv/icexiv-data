#import "../_components/pele/lib.notc" as pele;

# PBD 来源与验证

## 输入矩阵来源与独立求逆数学

### 静态来源调查

executable、GNU objdump 及授权边界沿用本页；没有启动、附加、仿真或调用
目标代码，也没有读取额外第三方实现。调查区间包括
`14025aa70..14025ab40`、`140267f88..1402682b0`、
`14020bbb0..14020bd20`、`14020b880..14020ba30`、
`14025b970..14025b9f0`、`14025bad0..14025bbb0`、
`140313710..140313890`、`140313a80..140313c10`、
`14178c0c0..14178c430`、`141efab70..141efad60`、
`141efb090..141efb1f0`、`141f41be0..141f41d90`、
`141f5bec0..141f5bfe0`、`14033ae00..14033afa0`、
`14033a7d0..14033ab50`、`140050ee0..140050f60`。
另读取相关 native reflection 名字/字段元数据与常量，不将其他对象的
相同 `+0xf8/+0x208` 字段合并解释。例如 `14020c780..14020ca40` 的
`+0xf8` 是另一对象的树相关字段，不用于证明矩阵来源。

可直接确认的消费数据流：

- `14026812c..14026815f` 将一个 u16 token 交给 `14025aa70`，复制返回的
  64 字节矩阵。token 高 4 位与低 12 位分开用于 part/bone 查询；它是
  此 runtime 调用的索引，不能由此把 MDL 文件骨表索引重命名为该 token。
- `14025aa70` 先取 part，并检查逐骨可用性位图。命中时由 `14020bc90`
  返回 part `+0x208 + bone×64`。`14020b966..14020b9bb` 可将调用者
  提供的 64 字节矩阵复制入该缓冲区并置有效位。因此不能把这个分支的
  所有 `G` 都直接命名为原始 reference pose、inverse bind 或动画姿态。
- 另一分支通过对象 `+0x58` 指针数组取所选 part 对应的数据，用其 `+0xc8`
  计数校验 bone 下标，从 `+0xf8 + bone×64` 返回矩阵。缺少指针和 bone
  越界还有不同分支；库不移植这些默认值与缓存策略。
- Skeleton resource 的 `140313b67..140313b98` 查找 animation container，
  将 `handle+0xc8` 传给 `14178c0c0`。初始化通过 container 的首个
  skeleton、parentIndices、referencePose 生成逐骨矩阵缓冲区，输出指针
  存在构造目标 `+0x30`，即 handle `+0xf8`。父索引和 referencePose 的
  native reflection offset 分别为 `0x18/0x38`，container skeletons
  为 `0x10`；这些名字由元数据单独校验，不仅凭偏移猜测。
- `14178c1ed` 进入父关系/TRS 累积助手；`14178c223` 生成矩阵，随后经
  `14033ae00 → 14033a7d0` 的仿射求逆写入该缓冲区。后者读取线性块的
  minors/determinant，除以 determinant，并生成相应负平移项；这里记录
  其代数角色，不将寄存器/共因子实现复制到库。TRS 累积的全部缩放策略
  和这些矩阵的运行期选择没有由本项执行对照。

资源构造与 fallback 读取的计数/指针布局相符，是 reference-derived
inverse-buffer 关联的静态依据。下节的资源 tag、factory 与 part setter
关联检查覆盖三个非零直接调用路径；其他表项别名、间接写入与运行期选择
没有穷尽，不能据此将每次 fallback 或每个 `G` 的空间一概命名。
缓存命中与后续条件修正又可改变输入。PBD 的 `G×D`、角色侧 child／模型侧
parent 和这项初始化线索仍不足以验收全局 source/target/bind 语义。

13 个关键区间及八个锚点并入 `client-pbd-observations.json`，静态复验共
35 段、41 锚点，另固定十个直接调用目标及三个 reflection 字段的名字/偏移。
复验仍检查五张实表全部 327680 个 ID，不把静态区间检查算成运行验证。

### 独立的通用仿射逆

`Matrix::try_inverse` 按普通矩阵代数独立编写：对线性 3×3 块使用 f64
partial-pivot Gauss–Jordan 消元，再计算 `-A_inverse × translation`。
固定栈空间，无堆分配、TRS 分解、native 缓存/分支、reference pose 替代
或 race 路由。这与本项已阅的 native 共因子/行列式实现不同；保留审阅接触
记录，不宣称严格 clean-room。

非有限输入、零计算 pivot、超出有限 f32 输出范围分别返回
MatrixInverseError 的 NonFiniteInput、Singular、OutputOutOfRange。
没有任意 epsilon、正则化或条件数阈值；病态矩阵可放大舍入误差。这个逆
仅是给定系数的数学逆，不把反序 race pair、EPBD selector、parent 路径或
逆绑定定义为同一操作。

`verify_pbd_inverse.py` 先核对完整实文件和既有 wire observer 输出摘要，
再对五张表全部 4215 个骨矩阵求逆。Python 将每个实际 binary32 系数转为
精确 Fraction，用共因子/行列式得到有理数逆，与 Rust 消元结果比较；
两侧不共用数值求逆方法。实表没有奇异矩阵，50580 个输出分量全部满足
`abs(rust-oracle)/max(1,abs(oracle)) <= 2e-6`，最大归一化误差为
`6.002304329976815e-8`，最大绝对误差为 `9.215145757579535e-8`。
该实验不调用客户端，不将数学结果称为 native reference-buffer 输出。

受控回归覆盖必须换 pivot 的非交换剪切/反射/平移、正反乘积与点往返、
奇异行、每个位置的 NaN/正负无穷、逆系数/逆平移的 f32 溢出和可逆小数。
`pbd-inverse-observations.json` 固定资源和报告 SHA-256
`8128839ff6308db8fb5a4a8db41f807ce25433c1d5308b0d46a290dcaee967f9`；
oracle 行摘要为 `982f190fe5fd9b7da7a87ef29e0fa0f8f93575e91ad5b3c324cba3a04aa26ed0`。
原始资源、逐矩阵输出和反汇编均留仓库外。

```sh
nix develop -c cargo build --features local --examples
python tools/verify_pbd_inverse.py /path/to/human.pbd /tmp/opencode/pbd-inverse
```

## Part 资源绑定与缓存输入

本节使用相同固定 Win64 executable，GNU objdump 2.46；没有运行或模拟
目标，没有咨询额外第三方实现。所有地址为该文件的首选 VA。验证脚本只
核对字节身份、PE 元数据引用和直接调用目标，不将缓存/装配过程移植进库。

### SKLB 请求与 fallback 指针

`140258040` 使用 owner `+0x50` 的 u16 数量分配两张指针表，分别存入
`+0x58/+0x60`，并初始化表本身。它没有初始化任何逐骨矩阵内容。
`140259110` 将第三参数存入 `owner+0x58[part]`，处理旧/新资源引用，再将
该资源传给 part 初始化。该表就是 `14025aa70` fallback 分支使用的表。

六个已识别的直接调用点中，三个传空指针，三个提供资源：

| 调用点 | 非零参数来源 |
| --- | --- |
| `14043bf3b` | 场景对象 `+0x300` 指针指向的待处理资源表，按 part 取项 |
| `14043c21b` | 同样从对象 `+0x300[part]` 取项的另一更新路径 |
| `140454ad8` | 上下文 `+0xb8` 所指对象的 `+0x100`，绑定至 part 0 |

资源身份另由请求到 factory 的链建立：

- `1404399b9..1404399d0` 把 `+0x300` 表、part、路径和资源 tag
  `0x736b6c62` 交给 `140430970`。后者在 `140430a28` 请求资源，
  `140430a2d` 将返回值写入对应表项。
- 第二组上下文在 `140454646` 使用相同 tag，`14045466c` 请求资源，
  `140454687` 将结果写入 `context+0xb8 -> +0x100`；它与第三个非零
  setter 调用的读取位置相同。
- `14031ba61..14031ba7c` 将该 tag 与 factory 表 `14214e868` 登记在一起。
  factory 的创建指针为 `140313da0`，其非零对象分支设置 vptr
  `14214e700`。该 vtable 的 `+0x80` 指向 `140313750`，返回类型名
  `Client.System.Resource.Handle.SkeletonResourceHandle`；`+0x108` 指向
  `1403138a0`，包含前述 skeleton/referencePose 矩阵缓冲区初始化链。

这三条路径的资源身份与上节资源构造可以关联，不只是在无关对象上看到
相同 `+0xf8` 偏移。此处确认的是请求、绑定和初始化来源；不证明某次
角色实例的分支选择或所有可达资源的生命周期内写入。

#pele.diagram(r#"""
flowchart TB
  req["SKLB tag / factory"] --> handle["SkeletonResourceHandle"]
  handle --> pending["pending table or secondary context"]
  pending --> setter["part setter: owner +0x58[part]"]
  setter --> fallback["fallback reads resource +0xf8[bone]"]
  ref["parentIndices / referencePose initialization"] --> inv["accumulation, matrix, inverse buffer"]
  inv --> fallback
  fallback --> input["G input to PBD matrix product"]
  cache["available per-bone cache"] --> input
"""#)

图中偏移为运行期对象字段的字节偏移，不是 PBD 文件偏移；对象别名和
动态空间不能由图中的箭头自动等同。referencePose 初始化链的数值缩放
细节与完整 source/target/bind 坐标仍分别保留证据边界。

### 逐骨缓存的两个调用输入

写入 `part+0x208[bone]` 并置有效位的方法入口为 `14020b830`；上节的
`14020b880` 是该方法内部的观察区间。缓存写入用第五参数提供的 64 字节
矩阵，同时保存独立的 TRS/辅助数据，不能只从字段位置给矩阵命名。

`1402eed0c` 与 `1402eef05` 是两个已识别的直接调用点。第一条路径从另一
对象 `+0x68` 指向的 stride=`0x70` 记录取矩阵，在 `1402eecdc` 经过已
识别的仿射逆助手 `14033a7d0`，再把结果作为第五参数传给缓存写入。
第二条调用的控制流也有取记录求逆路径，以及使用临时矩阵/TRS 运算的
另一分支。两处写入属于 `1402eec10` 的控制流；PBD 消费者在
`1402681a2` 按条件调用它，将已经读取的 `G` 副本地址作为第五参数传入，
位于后续矩阵乘积之前。记录的输入空间与动态用途没有由这些偏移证实，不能将所有
缓存值称为原始 referencePose 或直接动画姿态。

`verify_client_pbd_bindings.py` 对 29 个区间、27 个直接调用目标、八个
字段锚点、三个 RIP 相对数据引用和三个 factory/vtable 指针做身份检查。
`tests/fixtures/client-pbd-binding-observations.json` 固定来源和报告摘要：
`bec8596dd4843ddbe4f01f1a8262df3a8434a12084ba51377403011009064d28`。
反汇编保存在仓库外。六个 part setter 调用和两个缓存写入调用是已识别
直接边，不声明穷尽间接调用、内存别名或外部写入者；静态复验通过不计
为运行观察、完整空间解释或原生状态机验收。

```sh
python tools/verify_client_pbd_bindings.py /path/to/ffxiv_dx11.exe \
  /tmp/opencode/pbd-binding-static
```

### 条件上下文的 referencePose 输入

`14026802e/14026805f` 在 PBD 消费者中选择上下文构建路径。公共构建器
`1402f6490` 对 part 编号 0、3、4 的候选分支取 owner `+0x58[part]` 的
资源，再读取其 `+0xd0` skeleton 指针。part 0 分支另在 `1402f6544`
读取 deformer 请求记录 `+0x1a` 的第二个 ID，经 `14042d620` 查询 utility
资源；这个 ID 的角色／模型槽来源由后文的请求来源调查区分。utility
查询只有指定的一组 canonical ID 分支，不据此添加库内 race 回退策略。

`1402f6640` 把所选 skeleton 及多个上下文输出数组交给 `1402f1ba0`。
其中 `141efe510` 明确从 skeleton `+0x38` 复制每骨三个 16 字节向量，
该字段由独立 native reflection 元数据命名为 `referencePose`；随后存在
使用 `parentIndices` 的处理。初始化与活动动画姿态不能仅凭相似的 TRS
数组布局等同。上下文构造还包含骨名查找、part 对应和条件分支；这项
观察只确认 referencePose 是该初始化链的输入，不声称所有记录只是原始
参考姿态的简单全局累积。

记录构建的 `1402f2926..1402f29a4` 写入 stride=`0x70` 记录的尾部数值，
再调用 `1402f8a20` 生成前部矩阵。尾部 `+0x40/+0x50/+0x60` 分别承载
三分量平移、四分量旋转与三分量缩放；对齐字不据此命名。该记录随后
进入上述求逆／缓存写入链。referencePose、条件记录和缓存结果属于不同
阶段；这条来源链不把它们全部命名为同一个 inverse-bind/world 矩阵。

复验同时检查 `parentIndices/referencePose` 两个 native reflection 名字
和偏移，避免仅由 `+0x18/+0x38` 偏移相同推定类型。TRS 数值累积、part
映射优先级和实际运行分支仍不由静态摘要检查代替。

### 参考姿态的父累积与记录矩阵

固定 Win64 executable 的五个既有窗口共 1304 字节；直接调用
`1402f1cfb → 141efd6b0`、`1402f8a95 → 14033b4c0` 提供两个 helper。
前者的首个 PE runtime-function 范围只有 27 字节，同入口 688 字节补窗
覆盖条件返回目标；原件保留，新增唯一代码合计 1661 字节。两个显式 RIP
数据区共 20 字节，分别为四个 binary32 `0.5` 和一个 `1.0`。没有引用查询。
原字节、完整反汇编与独立审核保存在仓库外的
`pbd-reference-transform-21/`；这是离线静态证据，没有执行或模拟客户端。

#### 临时姿态与状态条件

`1402f1ba0` 建立临时姿态 P，骨数 n 来自 skeleton `+0x30`，scratch 为
调用者提供的 S。`P+8` 指向 S，`P+0x18` 指向 `S+48n`，分别作为每骨
48 字节的本地与输出 TRS；`P+0x28` 指向 `S+96n` 的逐骨 u32 状态，
`P+0x40` 指向 `S+100n` 的标量区。这里的偏移是内存对象字节偏移。

`141efe510` 从 skeleton `+0x38` 复制每骨三个 16 字节向量，逐骨状态写 2，
再把 `P+0x38/+0x39` 写为 `00/01`，最后清零标量区。`141efd6b0` 在
`P+0x39 != 0` 时直接返回。将这个调用判为跳过处理，需要 `01` 保持到
比较点；零填充区或其他数组若破坏性别名到状态/指针，该条件不能成立。
合法计数、容量、对齐与父输出就绪也由调用环境提供，前缀不验证这些条件。
`referencePose/parentIndices` 名称沿用本页的 reflection 证据，此项未重取名字表。

#### 父关系的理想代数

`141efd960` 检查总状态与逐骨 bit1，按骨下标递增处理。父 WORD 为 `0xffff`
时复制本地 TRS；其他值有符号扩展后用于父记录寻址，没有在该 helper 内
验证范围、拓扑或环。完成项清除 bit1，末尾置总状态。bit1 指数值的最低位
为 bit0 的第二位，与小端字节序分开定义。

设本地值为 `(t_l,q_l,s_l)`、所取父输出为 `(t_p,q_p,s_p)`，四元数采用
xyzw、Hamilton 积与列向量。所选指令可作以下普通实数代数识别：

- `q_o = q_p ⊗ q_l`，没有在此循环重新归一化。
- `s_o = s_p ⊙ s_l`；实际向量乘法覆盖四个 lane。
- 若 `q_p=(u,w)`，则
  `t_o = t_p + 2[(u·t_l)u + (w²−1/2)t_l + w(u×t_l)]`。
  单位四元数时等于 `R(q_p)t_l+t_p`。

平移段没有显式读取或乘父 scale；父 scale 在另一段参与缩放字段相乘。
该局部事实不能排除输入预处理、其他分支或中段的缩放修正，也不把父累积
等同于一般逐级 `T×R×S` 矩阵相乘。

#### 矩阵意义分量与导出来源

`1402f8a20` 把记录 `+0x60/+0x64/+0x68` 放到三对角，其余九个 XYZ 分量
置零，再把 `record+0x50` 的四个数交给 `14033b4c0`。helper 对四个
16 字节组各自的 XYZ 作同一四元数二次式变换；返回后将
`record+0x40/+0x44/+0x48` 加入第四组。每组布局如下，图中坐标以 **bit**
为单位，相对该组起点；各标量使用小端存储，组起点为记录的
`0x00/0x10/0x20/0x30`：

#pele.diagram(r#"""
packet
0-31: "X: binary32"
32-63: "Y: binary32"
64-95: "Z: binary32"
96-127: "W: unassigned stack bits"
"""#)

若四组 XYZ 视为 3×4 的四列，单位 q 的理想关系为
`[R(q)diag(s) | t]`。四个 W 槽在所选 builder/helper 中未初始化或计算，
64 字节复制会携带这些栈位，不能据此填成 `(0,0,0,1)`。非单位 q 时，
上节平移多项式矩阵与本 helper 的 R 相差 `2(‖q‖²−1)I`；两处公式不能
无条件互换。这里没有做 binary32 执行对照，乘加舍入顺序、非有限值和
MXCSR 行为不由实数恒等式消去。

`1402f2926` 导出窗口的旋转四分量来自入口 `(xmm6.xyz, xmm2.w)`；
`shufps xmm2,xmm6,0xff` 的低 lane 取旧目的寄存器的高 lane。窗口前的
寄存器定义不在这五窗内，单凭它们不能推定四分量同源。前缀到导出之间
相隔 3100 字节，其连续窗口证据见下节。调用矩阵构造前还重新读取输出
指针槽，两次指针相等需要该槽未被改写。因此局部 TRS 关系不能命名完整
source/target/bind 空间或所有缓存输入。

独立审核结果为 501322 字节，SHA-256
`fca14e5081dd44c85985149c3bcacde814b4722345b2840a7569ebc1f7143920`。
`tests/fixtures/pbd-reference-transform-observations.json` 固定八份代码窗口
（含重叠原件）、两份数据区、审核身份与限定。审核器对回固定 EXE、PE 映射、
全部 raw/asm 和完整结果对象/规范字节；人工代数判断单列，不把字节复验当成
自动空间证明。完整清单入口为外部目录的 `python3 -B verify_bundle.py --replay`。

### 条件记录的生成与导出来源

固定连续窗口 `1402f1ba0..1402f29a6` 共 3590 字节，包含上述 362 字节前缀、
3100 字节中段和 128 字节导出。两个重叠片段逐字节相同。调查与上节共用
4 入口/8192 新增唯一代码字节、2 数据区/256 字节和零引用查询预算；
累计为保守 3 个新增或扩展入口、4761 唯一代码字节、2 区/20 数据字节。
RIP 引用中仅使用已捕获的四个 0.5，其他常量内容没有读取。

#### 数组身份与分支

设 E 为构造器入口栈指针。`E+0x48/E+0x50` 提供两份 stride `0x70` 记录
数组的句柄，记为 A/B；`E+0x58/+0x60/+0x68` 提供另外三个 stride 48 数组
的句柄。F60 接收临时 pose 的父累积输出，L68 接收按状态同步后的本地区；
两者的目的下标是重映射结果，不能直接当成原骨下标。

前段 copy-forward 对 A/B 各复制 104/112 字节，跳过 `+0x4c..4f` 和
`+0x6c..6f`，另复制状态、元数据及三个 stride 48 数组，再回到状态检查。
它不证明任意重叠安全。所选代码的 count 检查针对 A，B 和三个 stride 48
数组的容量仍依赖调用契约。

允许新建记录的路径先用 F60 的 T/Q/S 构造栈矩阵，随后 A 有三种可见来源：

| 条件 | A 记录的可见处理 |
| --- | --- |
| PBD 查询返回非空 | 十二个 f32 重排成四组 XYZ，与栈矩阵一起传给 `14033a460`；指定 helper 的乘法方向见下节 |
| 查询无值，关联下标非负 | 读取关联 A 记录和 L68 的 TRS，组合后调用矩阵 builder |
| 查询无值，关联下标为负 | 将栈矩阵前 64 字节复制到 A |

查询读取请求 `+0x10` 资源指针及 `+0x18/+0x1a/+0x1c` 三个 u16；资源
非空才调用 `1417fa000`，其返回空时再调用 `1417fa110`。资源为空会跳过
两次查询。第一、第三条分支经 `1402e9c10` 返回指针，从 `+0/+0x10/+0x20`
取 T/Q/S 分量写到 A 尾部：只观察到 40 字节读取，不证明返回对象恰好
48 字节，也不认证该 helper 的分解策略。第二条分支不经过此 helper。

关联组合路径的 `1402f2674` 明确先计算
`v = S_associated.xyz ⊙ T_local.xyz`。四元数准备后的值记作 `q*=(u,w)`，
普通实数代数角色为：

- `T_out = T_associated + 2[(u·v)u + (w²−1/2)v + w(u×v)]`；
- `Q_out = q* ⊗ Q_local`，关联四元数在左；
- `S_out.xyz = S_associated.xyz ⊙ S_local.xyz`。

因此，中段有缩放作用于平移的正向证据。q* 的准备使用平方和、`rsqrtps`
及 `142548ea0/142548eb0` 两份未读常量，不能猜常量值或断言 q* 单位化。
这些公式不代表 binary32 运算、近似倒平方根或非有限输入的数值验收。

#### B 的混合来源与四分量同源条件

后段以一个 part byte 小于 5、一个有符号 WORD 非负门控 B 的生成。通过时，
T 从 A 的 `+0x40/+0x44/+0x48` 进入 XMM7/8/9；Q 从 A 的 `+0x50` 进入
带平方和/`rsqrtps`/未知常量的准备路径，结果位于 XMM6。S 则从临时 pose
按 bit1 选择的指针 `+0x20` 取得：清除时直接取父累积区，置位时由
`141efea30` 返回指针。该 TEST 与 JNE 之间的 SSE 指令不改整数条件标志。

#pele.diagram(r#"""
flowchart TB
  f["F60 TRS"] --> build["stack matrix / conditional A generation"]
  l["L68 TRS + associated A"] --> build
  d["optional PBD query result"] --> build
  build --> a["A record: translation and quaternion input"]
  a --> b["gated B TRS export"]
  p["pose output or pointer helper: scale"] --> b
  b --> m["matrix builder: twelve meaningful components"]
"""#)

图中箭头表示所选路径的字段来源，不是文件偏移或完整空间变换公式。
`1402f2923 movaps xmm2,xmm6` 在 helper 之后、旧导出窗口之前，明确让
随后沿顺序进入的导出取同源 `(x,y,z,w)`。把 Q/T 进一步归到调用前的
准备结果，需 Win64 ABI 对 XMM6..15 低 128 位的非易失约束；该未知 callee
没有被读取来证明它服从约束。A/B 多次解引用、数组与状态还需非破坏性
别名条件，不能把 B 整体称为未经修改的 referencePose。

独立审核对回完整 830 条指令、20 个直接调用、51 个分支、19 个 RIP 引用；
五个窗口外分支目标和 15 个未读 RIP 地址保留缺边。原字节、完整结果及
人工角色判断保存在外部 `pbd-context-export-22/`，结果为 2039532 字节，
SHA-256 `b7e11911a2aa1a11632da8f69e15b38bafe43779ed2991bddeaefba80ffa54f1`。
`tests/fixtures/pbd-context-export-observations.json` 固定身份和共享预算。
只读全件复跑入口为该外部目录的 `python3 -B verify_bundle.py --replay`。
完整 source/target/bind 命名、未知 helper、运行路径和四个 W 的含义分别
保留证据边界，完整渲染目标不由这段静态观察替代。

### 有值分支的仿射乘积与 W

直接边 `1402f25f3 → 14033a460` 对应 868 字节的固定 helper 窗口
`14033a460..14033a7c4`。入口 `RCX=O` 为输出、`RDX=L` 与 `R8=R` 为输入；
三个基址寄存器保持不变，正常返回 RAX=O。所选范围没有内部 call、jump
或 RIP 数据访问，完整字节及五字节调用边均与固定 EXE 相符。

每个 16 字节组的 XYZ 视为 3×4 矩阵的一列，前三列为线性块、第四列为
平移，以列向量作用。十二项人工核对给出的普通仿射代数为：

```text
O_linear      = R_linear × L_linear
O_translation = R_linear × L_translation + R_translation
O             = R × L
```

22 的有值调用以 F60 派生的栈矩阵 F 作为 L，十二个返回 f32 的重排区 D
作为 R，因此该路径为 **`A[d] = D × F`**。D 的四列是 `(f0,f4,f8)`、
`(f1,f5,f9)`、`(f2,f6,f10)`、`(f3,f7,f11)`，保持返回数据原行列关系。

#pele.diagram(r#"""
flowchart TB
  d["R8: D, left factor"] --> product["affine product D x F"]
  f["RDX: F, right factor"] --> product
  product --> output["RCX: A record XYZ columns"]
"""#)

图中箭头是参数/运算关系，不是文件坐标。这里的 F 与其他消费点的 G 属于
不同阶段；不能仅从 `D×F` 与 `G×D` 两式推出完整 source/target/bind 空间。

两个输入各读十二个 DWORD，输出写十二个 DWORD，字节偏移集合完全相同：
`00/04/08, 10/14/18, 20/24/28, 30/34/38`。每个基址实际访问 48 字节，
包围跨度为 `[0,0x3c)`；这不证明对象大小恰好 64 字节。矩阵基址的
`0x0c/0x1c/0x2c/0x3c` W 槽没有读写，不能由数学上的隐含第四行给这些
物理字节赋值。

最后输入读取 `14033a5f9` 早于首输出写入 `14033a632`，因此矩阵区相交
不会让输出覆盖稍后才读取的输入系数；仍需有效地址/权限、无并发改写，
以及与 helper scratch、保存区、shadow、返回地址之间没有破坏性别名。
矩阵标量访问本身不要求 16 字节对齐，栈上 `movaps` 保存/恢复另有 ABI
对齐前提。若矩阵错位相交，一个输入的 W 地址可能重合于另一矩阵的 XYZ
地址，故 W 声明必须按各自基址理解；下游 `1402e9c10` 是否读取 W 仍未知。

独立人工表覆盖全部 36 个 `mulss`、27 个 `addss` 与十二个输出 store。
线性第二列先加 k=1、k=0 两项，其他列的次序有所不同；实数乘积身份不
允许重排 binary32 运算，也不消除 NaN/Inf、±0、非正规、溢出/下溢或
MXCSR 行为。本项没有执行、模拟或数值求值客户端函数。

原件与完整审核保存在外部 `pbd-context-matrix-23/`。审核结果为 323763
字节，SHA-256 `dcffab31460c8915f0a42c621cff36b50da407f6389271e63a79fb97e1627aa6`；
`tests/fixtures/pbd-context-matrix-observations.json` 固定窗口、访问集合和
证据限定。全件只读复跑为该外部目录的 `python3 -B verify_bundle.py --replay`。
21–23 的共享累计为 4/4 入口、5629/8192 新唯一代码字节、2/2 数据区共
20 字节、零引用查询；入口/数据区数量已耗尽，不以剩余代码字节另开范围。

### 四种参考姿态差值假设的实数据核对

`observe_pbd_reference_relations.py` 使用同版本 `human.pbd` 和 c0101、
c0201、c0901、c1801 四份已登记 SKLB。输入 SHA-256 在
`pbd-reference-relation-observations.json` 固定；PBD 的全部独立 wire 行
另与既有摘要核对。三对关系为 `201→101`、`901→101`、`1801→201`，
parent 关系通过 PBD connection 下标校验，不从编号算术猜测。

该实验定义 A 为 parent、B 为 child 的标准层级矩阵：每条 local TRS
使用归一化 xyzw quaternion、`T×R×S`，列向量，按根到骨的顺序以 f64
相乘。它是明确的数学假设，不声称复现 Havok 的全部缩放／累积规则。
对全部同名骨比较四个候选：`A×inverse(B)`、`inverse(A)×B`、
`B×inverse(A)`、`inverse(B)×A`，与 PBD 中实际保存的矩阵逐系数比较。

三对分别匹配 103、105、100 个骨，没有丢失名字或最近似名称匹配；共
1232 个候选、14784 个系数比较。每对的四种假设都存在反例：其中最接近
的假设仍有约 0.307103、3.660129、0.242913 的最大绝对系数差；完整报告
包含每种假设的误差及见证骨名。因此这四种直接差值公式不能作为这些
实文件 PBD 系数的替代。该结果不排除其他 reference-derived 构造，也
不证明真实 PBD 应用空间或动画行为。

报告 SHA-256 为
`7f22ee11b30082cb92ab7b6f2b0a809205869b5e75055e45fcc7121597d050d7`。
原始资源和完整候选矩阵留在仓库外。联验把此项列为假设／库存观察，
不增加成功解码或坐标语义验收计数。

```sh
python tools/observe_pbd_reference_relations.py /path/to/human.pbd \
  /path/to/skl_c0101b0001.sklb /path/to/skl_c0201b0001.sklb \
  /path/to/skl_c0901b0001.sklb /path/to/skl_c1801b0001.sklb \
  /tmp/opencode/pbd-reference-relations
```

## 来源与许可证

- VFXEditor commit `ed6b39dcca04225c15c404a35a46945119462015`，先读取根 LICENSE：
  MIT，Copyright 2021 Michael Kaminsky。随后阅读
  `Formats/PbdFormat/{PbdFile,PbdDeformer,PbdBone,PbdConnection,TransformMatrix}.cs`，
  包括读写、矩阵转换和文件内的 UI 方法；
  `Extended/{PbdExtended,PbdEpbdData,PbdEpbdEntry}.cs`、
  `Flatbuffer/Schemas/epbd_schema.fbs` 与 `Utils/PackStruct/{PackReader,PackHeader,PackFooter,Pack}.cs`。
  它们提供公开字段命名和 EPBD schema。PBD/PackStruct 文件注释引用
  Penumbra.GameData，未知上游许可限制沿用 [材质记录](materials.md)，不视为已审查上游。
- aers/FFXIVClientStructs commit `b69073ae2d789527fc1b9a78aa7aa590e0f299dc`，
  先读取 MIT LICENSE（Copyright 2021–2023 aers），再阅读
  `Client/Graphics/Scene/CharacterUtility.cs`、
  `Client/System/Resource/Handle/{EquipmentParameterResourceHandle,PreBoneDeformerResourceHandle,CharaMakeParameterResourceHandle}.cs`
  （路径均位于 `FFXIVClientStructs/FFXIV/`）。这些声明确认资源角色，没有给出 CMP/EQP 字节布局；
  未作为它们的格式证据。

未复制、翻译或照搬上述解析/矩阵实现。Rust 使用受检借用视图、独立 FlatBuffers
wire reader、共享预算和连接森林校验；没有 FlatBuffers 或 FFXIV 解析库依赖。
不读取 legacy，不宣称严格 clean-room。

## 原始样本

用户授权只读 Win32 安装 `2026.10.06.0000.0000`，资源
`chara/xls/boneDeformer/human.pbd`，256228 字节。
文件和全语义行 SHA-256 固定在 `tests/fixtures/pbd-observations.json`。
主表 31 个变形器，头和连接表共 624 字节。EPBD/PACK 从 187872 开始，
总长 68356 字节，FlatBuffers payload 长 68316 字节。

## 独立观察与覆盖

`tools/verify_pbd.py` 使用 Python struct 独立展开主表与四个 EPBD 内嵌 PBD，
逐行对比 Rust 的 skeleton ID、两个方向的索引、全部链接、骨名字节、scale 位和
每个矩阵的 12 个浮点系数位。覆盖 5 张 PBD 表、46 个变形器、4215 个骨矩阵、
50580 个浮点系数；4118 个非单位矩阵，4112 个含平移、4076 个含非对角系数。
不是仅验证单位矩阵或骨名数量。

真实数据同时检查变形器/连接逆映射、parent 无环及 child/sibling 的 parent 一致性。
Rust 检查每个非根节点确实属于父节点 child 链，不使用 rest 骨架近似替代文件系数。
独立全位比较证实数据定位与存储行序，不证明游戏中的 deform 应用空间、方向、
矩阵链乘法顺序或 EPBD 条件求值；祖先链分支的静态顺序另见下文。

受控测试验证非平凡 3×4 到列主序 4×4 重排和点变换、重排连接表、缺省 payload、
嵌套 EPBD、共享预算、畸形 vtable/offset/字符串/图关系和确定性字节变异。
截断的非零追加数据返回错误，不能被解释为“没有扩展”。

## 复现

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_pbd
target/debug/examples/local_sqpack /path/to/game/sqpack \
  chara/xls/boneDeformer/human.pbd /tmp/opencode/human.pbd
python tools/verify_pbd.py /tmp/opencode/human.pbd /tmp/opencode/pbd-check
```

输出目录应不存在，原始资源与逐矩阵输出只保存在仓库外。验证 Python 3.14.8。

## Win64 标量与关系的离线静态证据

目标为获授权安装的 `game/ffxiv_dx11.exe`，51910400 字节，SHA-256
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`，
PE 首选 image base `0x140000000`。安装版本文本为 `2026.10.06.0000.0000`。
环境限制见 [CMP](cmp.md)：REA 原生引擎不可用，使用安装中的 GNU objdump 2.46。
以下为普通离线静态观察，不是运行观测或 REA Evidence；没有启动、附加、模拟
目标程序，没有更改安装或配置。地址是首选 VA，不是运行时地址。

- `1404267ad..1404267df`：PBD 路径用于资源请求，返回 handle 存于 utility
  `+0x200`。这与 [EQP 的 utility 身份证据](eqp.md)相接。
- `1417fa2d0..1417fa330`：从 handle `+0xc0` 取得 PBD 字节，按头部数量和
  `4+12*i` 的 skeleton ID 查找记录，返回记录 `+8` 的 f32。缺少 ID/数据返回
  常量 `1.0`，但 utility 包装层 `14042d280` 在没有 handle 时返回另一个常量，
  位模式 `0x3fc34fb5`，即 `1.5258699655532837`。二者不是同一种缺省状态。
- `140441fa0` 的上下文从对象 `+0xaa0` 保存一份 skeleton ID；模型槽 0–9
  的另一 ID 来自所选记录 `+0x16`，特殊槽另有分支。因此没有把所有槽都推定成
  相同的 ID 来源。`1404420df..140442131` 两次调用标量查询，保存第二次/第一次
  的比值至对象 `+0xc68+4*slot`。在这里是角色 ID 标量除以所选模型 ID 标量。
- `140442855..140442885`：按模型槽取该比值，传入 `14042c740`；后者把它
  写入参数数据的首个 f32。utility `+0x530` 保存 `g_ModelParameter` 的动态
  名字索引，相应对象存入线程 context `+0x940[index*8]`。
  [具名 shader 消费链](pbd-scalar-consumer.md)将名字登记、参数字节、packet
  读取与 SHPK/RDEF/SHEX 关联：七包的 316 个 VS 使用 `m_Params.x` 参与
  `m_Wetness` 运算，其中一个条件路径的数据依赖到达 `SV_POSITION`。
  原始 scalar 的物理定义与具体绘制的实际选择／绑定仍无独立证据。
- `1417f95b0..1417f9699`：第一个 ID 的 connection→parent→deformer ID 与
  第二个 ID 比较，确认直接 parent。`1417f94b0..1417f95a3` 从第一个 ID 的
  parent 起逐级比较第二个 ID 对应的 deformer index，确认严格 ancestor。
  这些关系不确定 3×4 矩阵的乘法方向或空间。

阅读还包括 `1417f9fb0..1417fa2d0` 的类型名字和条件分支，以及
`1417f96a0..1417f9840` 的骨名/48 字节记录定位；EPBD 分支选择和矩阵应用
的跟踪边界见下节。客户端的树查询、关系判定、比例求值、参数
分配或矩阵消费过程未复制、翻译、模拟或移植到库中。Rust 保留原始标量和
既有独立 ID 查询；不增加猜测的比例或骨变形求值 API。不读取 legacy，
不宣称严格 clean-room，不推定本地专有程序可再分发。

### 文件查询复验

`verify_client_pbd.py` 校验 executable 身份、十四个指令区段摘要、十九个
字节锚点、路径 LEA、调用目标和两种缺省常量的 RIP-relative 引用。
固定记录在 `tests/fixtures/client-pbd-observations.json`，全反汇编留在外部目录。

同一脚本使用独立 Python struct 观察器展开真实主表和四份内嵌表，再与 Rust
`find_deformer` 对照每张表的全部 65536 个 u16 ID。共 327680 次查询，46 次
命中，其中 5 次有记录但没有矩阵载荷；标量位、连接索引和骨数量全部相同。
原始全矩阵/关系输出摘要也必须匹配 `pbd-observations.json`。API 的缺少 ID
仍返回 None，不套用客户端的两个缺省常量；文件中的 0 和 65535 ID 没有保留
规则推定。受控回归检查这两个 ID 与无载荷记录的非单位标量。

主表 31 个标量的实际范围为 `0.6966300010681152..1.9417799711227417`，
无载荷记录 101 的位模式与包装层的缺少 handle 常量相同；另外五个主表 ID
也具有相同值，不能从相等值推断骨载荷缺失或同一种族。

```sh
nix develop -c cargo build --features local --example local_sqpack \
  --example validate_pbd --example validate_pbd_lookup
target/debug/examples/local_sqpack /path/to/game/sqpack \
  chara/xls/boneDeformer/human.pbd /tmp/opencode/human-pbd-static.pbd
python tools/verify_client_pbd.py /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/human-pbd-static.pbd /tmp/opencode/client-pbd-check
```

完整 source/target 空间、逆向变换和 EPBD 条件求值仍为语义缺口；祖先链分支
的有序乘积与一个模型矩阵消费点见下节。

## 祖先链与矩阵消费的静态边界

同一固定 executable 的以下片段扩展了标量调查，仍只用离线 objdump。没有
执行本地程序、模拟其函数、复用其缓存或将 native 指令翻译为实现。

- `1417f96a0..1417f982d` 首先检查两个 ID 的直接 parent 关系，然后读取第一
  个变形器的载荷，按骨名匹配并返回对应的 48 字节矩阵。名字查询包含缓存与
  线性分支；二者都指向文件矩阵，不以 rest pose 替代。
- `1417f9840..1417f9a67` 检查严格 ancestor，初始化仿射单位矩阵，读取起始
  记录中的同名骨矩阵，经 `140341430` 累积，转到 parent 记录；若到达指定
  ancestor，则在读取该 ancestor 载荷之前停止。找不到名字返回零指针，不能
  据此构造单位矩阵成功。EPBD 的优先分支仍由外围条件选择。
- `140341430..140341775` 是通用行存储仿射乘积。以左侧内存 `L`、右侧 `R`
  解释，首项写回 `L00*R00 + L01*R10 + L02*R20`，平移为
  `L00*R03 + L01*R13 + L02*R23 + L03`，其他行列同样取行与列的点积。
  因而上面的累积顺序是 `D_start * D_parent * ...`，不包含终止 ancestor
  记录。此为指定分支的代数约定，不是全局路由规则。
- `1417fa110..1417fa2c6` 包含 EPBD 查询、直接关系检查和上述祖先链调用，
  没有把其中的条件树或 fallback 算法移入库。
- `140267f88..140268451` 的模型消费过程按模型骨名查询矩阵，调用
  `14025aa70` 取得输入 4×4 矩阵并保存；条件修正过程还可修改输入或 PBD
  矩阵指针。`140268283` 之后将输入矩阵、PBD 矩阵和输出地址传入
  `140341220..14034142a`。输入矩阵按列存储，PBD 与输出按行存储，所得
  代数关系为 `G * D`。没有 PBD 矩阵的另一分支仅重排 `G` 的前三行。

这里只确认这个消费点的乘积和存储约定。`G` 可能经过姿态/骨架和额外条件
路径，未完整追踪其坐标空间、动画/绑定变换或 source/target 路由，故没有
声称 world/local/bind 空间已全部验收。外围未选取分支也不由本证据代表。

静态摘要、关键的 parent/名字定位锚点及调用目标仍由
`tests/fixtures/client-pbd-observations.json` 与 `verify_client_pbd.py` 固定。
完整上下文保存在外部 `client-pbd-chain-01.asm`、`client-pbd-consumer-01.asm`
及复验目录，不再分发客户端反汇编。

### 独立仿射数学验证

`Matrix::compose` 依普通仿射矩阵乘法定义独立编写：三行与右矩阵各列取点积，
平移列加上左矩阵平移。它没有 native 的寄存器/栈布局、缓存、路由或条件结构，
不选择链或求值种族变形。受控测试用非交换的剪切/平移矩阵验证顺序、单位矩阵
和点变换复合关系。

`verify_pbd_composition.py` 以既有独立 wire observer 的全部真实矩阵系数
构造带 `(0,0,0,1)` 第四行的 4×4 矩阵，使用 Python f64 的四项点积，与 Rust
f32 三行仿射操作比较。每个载荷按存储顺序相邻配对（末项回到首项），这些
是任意数学测试对，不冒充相邻骨、连接路径或实际角色变形。

五张表共 4215 对，其中 3801 对的正反乘积不同。正反乘积全部 24 系数及
每对三个点的九个分量共 139095 个数一致，判据为
`abs(rust - oracle) / max(1, abs(oracle)) <= 2e-6`。观察到最大归一化误差
`4.131547864938412e-7`，最大绝对误差 `5.396020494430331e-6`，没有改变容差。
文件身份、配对计数与独立 oracle 摘要固定在
`tests/fixtures/pbd-composition-observations.json`。数学验证不代替空间语义证据。

```sh
nix develop -c cargo build --example validate_pbd_composition
python tools/verify_pbd_composition.py /tmp/opencode/human-pbd-static.pbd \
  /tmp/opencode/pbd-composition-check
```

## 文件内祖先路径的双向验证

`Pbd::ancestor_path` 是独立编写的文件森林查询，接受两个变形器表下标，返回
从 descendant 向 ancestor 的有序表下标（不含 ancestor）。它通过记录中的
connection index 和 parent 字段导航，不使用 native 的指令、缓存、判定或
回退结构。没有逐种族表、EPBD 条件策略、骨名匹配或矩阵求值代码。

Rust 查询预先证明关系存在，并为第二次迭代保留逻辑记录读取预算；查询与
迭代均不分配路径缓冲区。self 路径为空，跨根或反向关系返回 None，非法下标
和预算不足是独立错误。这里的 self 空路径是文件图查询定义，不冒充客户端的
strict-ancestor 布尔判定。查询不确定哪个 skeleton 是应采用的源或目标。

`verify_pbd_paths.py` 重用 Python struct 的独立 wire 观察，从每个候选 ancestor
出发沿 **first_child/next_sibling 广度优先遍历**，以向下路径反序得到预期结果，
与 Rust 的 parent 向上遍历比较。两侧的关系来源和遍历方向不同，不以 Rust
自己计算的父路径充当 oracle。全部原始矩阵/关系输出摘要先与既有 PBD 固定
记录匹配，且主表与四份内嵌表分别处理，不把内嵌局部 ID 改成外层 race code。

五张真实表的所有 1032 个有序下标对完全一致：46 条 self 空路径、919 对无
关系、67 条非空路径；非空路径共 93 个节点，最长两节点。结果与源文件摘要
固定在 `tests/fixtures/pbd-path-observations.json`。这证明该真实文件中的全部
路径元数据，不证明它们都被客户端选用。

受控回归覆盖重排后的两张索引表、兄弟分支、独立根、反向查询、空路径、越界、
预算不足、迭代耗尽，以及反序连接表中的 63 节点长路径。长链的预算在返回
迭代器前检查，不通过反复 `next` 绕开该查询的记录读取预算。

```sh
nix develop -c cargo build --example validate_pbd_paths
python tools/verify_pbd_paths.py /tmp/opencode/human-pbd-static.pbd \
  /tmp/opencode/pbd-path-check
```

## EPBD 根标量与局部 selector

### 审阅线索与独立 native 观察

沿用[外部实现审阅](external-volume-review.md)固定的 Meddle commit
`383bdf82d2e35741039f1474c2c9f475f2cec9f9`、根 AGPL v3 许可证及 README
来源审查，再阅读 `Meddle/Meddle.Utils/RaceDeformer.cs` 的 1–70 行，文件
SHA-256 为 `35d54c5faf69b1037590ab87b29a657e9b5442e745e89155c13d91ed5e647d42`。
它接受一个已经选好的 deformer，按骨名/骨父节点找矩阵并变换顶点；这段
没有解释 EPBD 根字段或局部 selector。它只用于界定调查问题，没有移植其
解析、骨回退或变形实现，也不将该行为当作客户端的完整选择策略。

独立观察继续使用本页固定 Win64 executable、PE 映射和 GNU objdump 2.46。
阅读范围包括 `1417f9a70..1417f9fb0`、`1417f9fb0..1417fa2c6`、
`1417fa330..1417fa830`、`1417fa930..1417faf50` 和
`1417fb0f0..1417fb4c0`。以下是静态字节/指令事实，不是函数模拟或运行观察：

- `1417fa500..1417fa663` 找 EPBD PACK、核对 FlatBuffers 标识和验证器，
  从根 vtable `+4` 取逻辑字段 0。若字段省略则继续；若存在，在
  `1417fa630..1417fa639` 以 **u16** 与 1 比较，不相等则不登记扩展。
- 根验证器 `1417fb0f0..1417fb2bd` 从 vtable `+4/+6` 取两个可选字段，
  分别以两字节宽度检查其边界。它们是根字段 0/1 的 u16 标量，不是指向
  子表的 uoffset。字段 1 的含义与缺省数值没有由此命名。
- `1417fa930..1417fae60` 从根字段 2 取得 entry 向量。entry 的字段 2 是
  u32 selector 向量；`1417fac06` 逐项读取 DWORD，`1417fac5d` 将其低
  u16 存入三字段键。另两个键分量来自 entry 的 child/parent 的低 u16。
  `1417facce..1417facdf` 用内嵌 PBD 指针、selector 的低 u16 及 parent ID
  查询 ancestor；登记的键指向该内嵌表。没有将 native 树结构或条件选择
  算法翻译进库。
- 查询包装 `1417fa000..1417fa106` 从请求 `+4` 读取 u16 selector，
  在其非零时检查扩展查询路径，并对内嵌表作骨矩阵查询。外围请求如何
  生成 selector、分派表比较与回退的完整策略没有由本证据验收。

四段核心 native 范围和七个字节锚点添加到既有固定清单；
`verify_client_pbd.py` 的复验覆盖 18 段、26 个锚点，同时保留全部 327680
个实际 ID 查询对照。源代码审阅后的独立 native 观察不改变“不宣称严格
clean-room”的方法声明。

### 实际字节与根字段受控实验

原始 `human.pbd` 的 EPBD root 位于 payload `+20`，vtable 位于 `+10`，
vtable 长 10、table 长 8，三个字段 offset 为 `[0,0,4]`。因此实际文件省略
两个根标量，不能以该文件本身证明显式字段的数值，也不能从省略推定它们
是子表。四个 entry 的八个 selector 全部匹配对应内嵌 PBD 的局部 skeleton ID：

| 外层 child / parent | selector IDs | 对应骨矩阵数 |
| --- | --- | --- |
| 1501 / 901 | 1 | 105 |
| 1601 / 201 | 1 | 100 |
| 1701 / 101 | 1 | 105 |
| 1801 / 201 | 1,2,3,4,6 | 各 100 |

每个局部记录的祖先链都包含该 entry 的 parent ID。此关联与 native 登记
时的参数来源一致，说明这些元素是局部记录选择编号，不是可按位 OR 的
条件掩码。具体外观状态的命名、是否所有记录都会被选中、绑定空间和逆向
变换语义仍分别保留边界。

`verify_epbd_scalars.py` 在原始根表的 entries 指针前插入四字节，构造五个
仅显式化根标量的副本，分别为 `(1,0)`、`(1,65535)`、`(2,48879)`、
`(0,65535)`、`(65535,1)`。后面的向量、矩阵和名字字节完整保留，所有相对
指针随目标一起移动。Python 直接读两字节字段，逐项与 Rust 可选值对照。
每个副本和原件的全部 4215 骨矩阵、关系、名字与标量语义行仍匹配原始
wire observer 摘要；八个局部 selector 对照也逐项一致。五个副本是实验
输入，不是游戏生产文件；tag 2/0/65535 的结构读取不宣称客户端会接受它们。

根字段 getter 分别为 `version_tag()` 与 `unknown_root_scalar()`，返回
`Option<u16>`，不合成 omitted 值。`conditions()` 保留原 u32 数据；
`local_skeleton_ids()` 显式暴露指定 profile 使用的低 16 位，不作 entry
选择或应用变形。Rust 回归另覆盖字段恰好结束于 table/input 尾部、相邻
u16 值、越界字段、截断、原始 selector 高 16 位保留及局部 ID 缺失。

完整实验报告 SHA-256 为
`9490d4a4337182d853e973e380d64f31a3fedb855214155260e3bd1ff7f0b476`，
固定在 `tests/fixtures/epbd-scalar-observations.json`。资源、副本、逐字段行
与反汇编留在仓库外。该证据修正根字段类型并明确局部 ID 关联，不声称
source/target/bind 空间或完整 EPBD 优先级已验收。

```sh
nix develop -c cargo build --features local --examples
python tools/verify_client_pbd.py /path/to/game/ffxiv_dx11.exe \
  /path/to/human.pbd /tmp/opencode/client-pbd-epbd
python tools/verify_epbd_scalars.py /path/to/human.pbd /tmp/opencode/epbd-scalars
```

## 角色模型槽请求来源与精确 pair 查询

### 审阅范围

继续按 Meddle 的已登记 commit 与 AGPL v3 根许可证阅读
`Meddle/Meddle.Plugin/Models/Structs/Deformer.cs` 1–24 行，SHA-256 为
`570b050f3c5ef84b9b052bd2d38b25bb22ebbe1c85e1ed1249024c1426405066`。
该 overlay 给出 `+0x10/+0x18/+0x1a` 的成员，但没有 selector `+0x1c`。
它不能单独证明缺少 selector 或给它命名；字段来源以下述独立 native 链为准。
没有复制 overlay 或引入 FFXIVClientStructs 依赖。

离线 objdump 阅读范围包含 `140267a70..140268190`、
`14025aa70..14025ab40`、`140272d70..140272e40`、
`140287440..140287550`、`1402b9470..1402b99a0`、
`140324490..1403246d0`、`14042adf0..14042b170`、
`140441fa0..140442155`、`140442620..140442750`、
`140443700..1404439e0`、`140444630..140444950`、
`1404452a0..140445440` 及 `14044f540..14044fa50`；另对外部汇编文件作
调用/字段写入位置搜索。并非这些范围中的每个分支都得到完整语义解释。
辅助/terrain 等其他调用者不用于推断角色的 selector 含义；没有因看到
相似 `+0x1c` 字段就把不同对象的含义合并。

### 可闭合的请求参数链

固定 executable 和工具身份沿用本页。所有地址为首选 VA，不是运行地址：

1. `140445320..14044540f` 的普通模型槽分支从角色对象 `+0xaa0` 取第一
   ID，从 `+0xc50` 指向的 32 字节槽记录 `+0x16` 取第二 ID，从同记录
   `+0x18` 取 selector。第一/第二 ID 放在 `r8w/r9w`，selector 作为第五
   参数，经 `1404453f2` 调用 `1402b97f0`。角色 ID 与模型记录 ID 的来源
   还与本页的逐槽 scalar 比值路径一致。
2. `1402b97f0..1402b9899` 保留两个 ID，将第五参数传给 `140267af0`。
   初始化函数先以两个外层 ID 检查主表严格 ancestor；通过后将资源指针
   存到对象 `+0x10`、第一/第二 ID 存到 `+0x18/+0x1a`、第五参数存到
   `+0x1c`。selector 不属于该函数初始的主表祖先判定。
3. `140267bd0..140267c54` 将这三个 u16 依次复制为六字节请求，先调用
   `1417fa000`，缺少结果时再调用 `1417fa110`。既有模型矩阵消费路径
   `1402680d9..140268129` 也从同一三个字段建立相同请求。
4. 资源查询包装与主表关系函数将第一个 ID 作为 descendant，第二个作为
   ancestor。EPBD 登记的三字段键是 child/parent/local selector，所以这条
   角色槽路径对应角色侧 child、所选模型侧 parent。

四段关键范围、七个参数写入锚点和五个直接调用目标纳入固定复验。
`verify_client_pbd.py` 共检查 22 段、33 个锚点，并继续对五张实际表做全部
327680 个 ID 查询。不执行或模拟 native 的初始化、缓存、特殊槽、优先级
和 fallback，也不将其条件树移入库。

证据只把 selector 追到所选模型槽记录，未把该字段的生成完整关联到外观
字节、模型路径或业务命名。ID 角色关联也不单独证明矩阵应用的 source/
target/bind 空间；`G` 的取值还有姿态、缓存及条件修正分支。没有由这条链
推出“反向 pair 等于求逆”或为所有槽套用同一规则。

### 独立文件查询

`Extended::find_entry(child, parent, max_entries)` 是文件元数据的独立线性
查询，比较完整 u32 身份，不复用 native 的有序树、低 u16 键转换或路由。
每读一条 entry 消耗一个预算单位；唯一匹配需扫描完整向量，无匹配返回
None，重复 pair 是查询歧义。预算不足为错误，不能把扫描未结束的匹配
当成唯一结果。重复 pair 本身不因此成为结构解析禁止项。

受控回归覆盖共享同一表的两个 vector 引用、正反 pair、宽 u32 身份、
空 entry 向量、已命中但无法扫描完的预算不足，以及重复匹配。原始 u32
高位不同的 pair 不因 native 使用低位而被混为一个文件查询结果。

`verify_epbd_scalars.py` 的原件和五个根字段副本各做八组 pair 查询：
四个实际 pair 均返回对应内嵌字节长度；反序、零 pair、u32 最大值及
`1801+65536,201` 均确认缺失。共 48 组 pair 对照；八个 selector 的关联
以及全部原始矩阵/名字/关系对照继续保持。oracle 从 Python wire entry
列表独立筛选，不调用 Rust 查询计算期望结果。

包含 pair 查询的完整报告 SHA-256 为
`bf72dbf52de1c464b6243719e748bea7616575e33fe3d0de37c5ca8a5ac4f634`，
仍由 `epbd-scalar-observations.json` 固定。上节的
`9490d4a4337182d853e973e380d64f31a3fedb855214155260e3bd1ff7f0b476`
标识仅含根标量/selector 的实验报告；两份报告的原始资源和受控副本身份相同。
复验命令沿用上节，完整数据和反汇编留在外部目录。
