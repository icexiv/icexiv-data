# 手锯挂点与动作时间线证据

来源为关联会话保留的手锯导出清单及其内容寻址对象、离线提取的四份 PAP。
只读资源字节，不提交游戏内容。REA 6.10.1 `execute-rizin-command` 以
`--base-address 0x0` 查看以下文件范围；两个调用均完整返回，前后文件摘要相同。

| 资源 SHA-256 | 命令 | Evidence |
| --- | --- | --- |
| `77f9749b65324ae4327e8ae0c3826e9dcd134d178fca7b45013a83c23eae4d0b` | `p8 488 @ 0x1447c` | `ev_f1edec428bd0128d47d6526eca0d3a46da64070e6b30979f4d4b3255d2468fa7` |
| `5c6b8c2673d33803822704f5655e7f1614da681a68731536426c09635689388a` | `p8 161 @ 8992` | `ev_31b13eb57307b627fc52ac38e5df22f0cd18c0527a29d8715d4d56365af5628e` |

第一段为整个 Bind，`BnVr=0`、`bFTO=ff`、`bTSc=01`。`Data/SpS` 的
`KeyC=1`、`BvPr=BvPo=RanT=0`，唯一的记录为 `(0,1,1.0,1.0,1.0)`。
同清单的 MDL 与 SKLB 传给 `validate_binder`，Start 只匹配一条 ElementId 和
一条 `n_hara`；ElementId 局部平移的浮点位为 `(0,3e3eaf97,0)`。
这支持数值输入的提取，不证明客户端如何解释标志或怎样移动粒子。

第二段是整个 `sit.pap` 时间线。另用 Python `struct` 从 PAP `+22` 取段起点、
沿各节点长度独立行走，再把偏移、标签、列表全部 ID 和 C009 名称逐行与
`inspect_motion` 比较。四份提取物共 12 个 TMLB、85 条记录、61 个引用值；
12 个 C009 名称与各包相同序号的 PAP 名称相符。

| PAP | SHA-256 | TMLB / 记录 / 引用值 |
| --- | --- | --- |
| bt_common_resident-idle | `96c29c780bad9d7cbccc2fe012ca2b1b34a950ca91df3e8b7180bc7dc1695d37` | 2 / 13 / 9 |
| event_base_chair_sit_relax | `c9805e3a243a6d121105c8de71368dc5089e772ae1dd326cdd8f9ba5102a6d96` | 1 / 8 / 6 |
| move_a | `89e733a21e3c47b5664435bc71e39b71eb8146c51c7fa0eca6feced73beb19ba` | 8 / 58 / 42 |
| sit | `5c6b8c2673d33803822704f5655e7f1614da681a68731536426c09635689388a` | 1 / 6 / 4 |

复验已提取文件：`cargo run --example inspect_motion -- <file>`。
四包交叉核对：`python tools/verify_motion_timelines.py <pap目录> target/debug/examples/inspect_motion`。
多包同序的观察不外推为所有 PAP 的强制位置关联。命令的播放时序、原生姿态混合、
动态挂点及最终像素没有运行时观测；父链仿射变换与稀疏重排使用受控数学回归。

## Win32 坐标查询与标量曲线

客户端版本 `2026.11.03.0000.0000`，`ffxiv_dx11.exe` SHA-256
`52042b60631cf2c48851b3aaee36e9ec4fedd6d7f2c68f0e9a8c3befb0af9f98`。
REA 6.10.1 / Rizin 离线读指令，另用 `objdump -d -M intel` 与磁盘常量核对。
地址为此映像 VA。以下是静态可达分支，不是运行跟踪。

| 范围或入口 | REA Evidence | 观察 |
| --- | --- | --- |
| `0x14039d260` 起 | `ev_1a0afdd6a0996a7606713211880087190484be3e6d560bb7edf1c85cbce80815` | 构造时 `+0x2a` bit 2 置位、`+0x29` 清零。`bFTO` 比较值是 `0x6246544f`，非零写 bit 2；减法分派累计到 `0x62545363` 即 `bTSc`，整字节写 `+0x29` |
| `0x1403c0994` 起 | `ev_d8fd5e006c0e1004b39df09f7cb63c606eb54cb83c64189879f9349d43b7d055` | binder 实例经 `+0x230` 取定义，将定义 `+0x29` 零扩展到实例 `+0x270` |
| `0x1403bff18` 起 | `ev_c4e8059a3873151a35196156ba177297c1e995f12dc471f51a83f500b5431712` | 宿主坐标查询成功后调用分解，`+0x270` 为零则以 `0x1428b5590` 的三项 1.0 覆盖长度；非零保留。另有距离与标量分支 |
| `0x1403c0300..0x1403c06c4` | `ev_cad46410de5d812522dd903c4177ca5803697b1a813b2503fa07eacd52fb035d` | 三条基向量各取 XYZ 长度。bit 2 开启时长度加常量 `0x34000000`（binary32 epsilon），用倒数缩放对应基向量；关闭时基向量取单位基。两条路径均保留平移 |
| 标量分派、插值 `0x1403977b0` 起 | `ev_6e6e1b33d29167b68980869f1b9314a8239931e85c00b1ffa46e32d1770382f1` | 由区间右键高两位选择函数，表 `0x14215a1e8` 依次指向 Hermite、线性、阶跃。阶跃比较到右端时换值 |

载入段 `0x140398870` 把键的第三个 f32 直接保存为值，前两项乘 15、截断为整数后存字节。
样条读取有符号字节并乘磁盘常量 `0x3d888889`。前后邻键存在时，切向量还乘本区间
时长占跨两区间时长的比例。边界表 `0x14215a230/248` 的三个函数为首末值、重复、
重复加周期值差。精确外部整周期在前方映射到末端，在后方映射到首端。
手锯 `Ptcl[6]/Col/A` 的 20、30 帧值分别为 `0.274509817…` 和 `1`；
`inspect_motion <手锯AVFX> 25` 得 `0.63725495`。这是粒子自己的 alpha 输入，
不等于锯身表面最终透明度。

RGB 指令证据：`0x14039cb87` 比较 RGB 标签并调用 `0x14039ab00`；载入循环
保留三个 f32。求值分派 `0x14039af02` 取右键类型。REA 对固定 executable 的
`pxq 96 @ 0x14215b4a8`（`ev_60f3f300ad84081b9f4d226fe3297495cf8f81d85fc4013e5667f2ed3e3e5d20`）
显示 0/1 指向相同线性函数 `0x14039af30`，2 指向阶跃函数 `0x14039afe0`。
前/后行为表的 1/2 分别共享 `0x14039b0d0` / `0x14039b1d0`。`0x14039ae91`
和 `0x14039aec2` 的分支包含首/末时刻。

`tools/verify_saw_motion.py <manifest> <objects> target/debug/examples` 验证手锯
12 个粒子的全部 `Col/RGB`，包含每个键、各相邻中点与区间外时刻，共 540 个
三通道样本，独立插值最大绝对误差小于 `5.4e-8`。同一导出物的
`element_by_id(3)` 和 `bone_by_name(n_hara)` 唯一命中骨 1。单位宿主帧下挂点为
`(0,0.18621668,0)`；宿主轴设为 `(0,2,0),(-3,0,0),(0,0,4)`，原点
`(7,8,9)` 时，挂点为 `(6.44135,8,9)`、缩放为 `(2,3,4)`。第二组是显式的
受控输入，不是游戏持握校正或实机逐帧采集。

**坐标更新窗口。** `0x14039cff2..0x14039d017` 将 CoUF 的低 16 位写入属性 `+0x22`；
`0x1403c2247..0x1403c2252` 有符号扩展后存为浮点截止帧。
REA `ev_d5eb2550653e62def602d89d6f7ca33bf94ba4cb4924d5f9d21874ec70e336d9`：
`0x1403c28cc..0x1403c291a` 在截止帧小于 0 或不小于传入局部帧时进行坐标查询；
反之跳过查询。这是已经进入该更新分支时的条件，不是调度器全局时钟。

**随机幅度。** REA `ev_410531ca2a1944d246816065c0438c79a853160c36873c1738e901c495270dd7`
覆盖 `0x140397f70` 的实例选择。每次抽样推进四个 u32 状态，取新末项低 16 位；
`RanT=0/1/2` 的双向/正向/负向比例乘 100 后向零截断，后续重用该有符号百分比。
`0x140398180` 的分派表 `0x140398388` 中前三项相同；3/4/5 则分别在求值时
从 `[-a,a]`、`[0,a]`、`[-a,0]` 抽取（`a` 为该帧幅度）。四字初态
`[1,2,3,4]` 的前七次低字值为 `2061,6175,4,8224,77,8378,142`。
`random_channel_draw_lifetime_and_seed_sequence` 分别检验首次抽样与复用/逐次抽样。
不据此假设生产客户端的种子或父子粒子更新次序。

## 外部动作资源链

中文 Action EXH/EXD 的 SHA-256 为 `25ac3175672f1ed8f35e38dd1a8ee121e9539dd9dd2b5406854a2ebff5a42268` /
`1e5c69b194b2f20344ae6d51cc790a5dd1ac9256065346b43ab255e4199dcba1`；
ActionTimeline EXH/EXD 为 `e03d9dad402ab1215d21c8c64c0023e5030f7085c92235732dd9897ca2e5dd8c` /
`f5cc45c6154bbb237e9e5d7bd7b98d20298d7526cb038758b7a30d2d62a0bc2b`。
前者第 7 列（从零开始）是固定区 `+32` 的 i16；后者第 6 列是 `+0` 处的字符串引用。
`action_key` 对 119/120/127/141 分别得到 448/449/456/473，
键为 `magic/cnj_white/{stone,carel,stonela}` 与 `magic/thm_black/fire`。
列布局和关系来自字节与资源路径匹配；没有采用无许可的外部表 schema。

飞石 `stone.tmb` SHA-256 `627f77d7c118c7b1978171dd7fe933959550885c4e4e0737cd50728e0456d8c5`，
REA `ev_8126ab47cabf7be4d022d8d7d190427659503f3a894a463d85c0d47b27c5f2bb`
完整 847 字节。按 C012 的尾区偏移读取的 AVFX 经 SqPack 定位、独立提取再由
`validate_avfx` 解析，25832 字节，SHA-256 `aea4162c14fb5aebe72422139f3cda4830a6e25ec8782dadd2c003a511956b92`。
其 C002 所指的 `magic_cnj_end.tmb` SHA-256 为 `c2e5910cb58cd4bef118d3f6a5e1e74b993cae28f45160160202b5b5c99f2b63`。

按 ActionTimeline 页内顺序从 `ability/` 与 `magic/` 每个父目录取首个 key，
40 个独立 TMB 共声明 85 个 C012，根列表可达 82 个。
`verify_action_effects.py` 独立从磁盘 ID 列表遍历，逐条核对 200 个可达资源引用的
位置、ID、时间、时长、资源种类和字串；输入清单聚合 SHA-256 固定在脚本中。
这些可达 C012 的 TRS 和颜色均为默认值；非默认数组、负起点和长时长由受控回归验证。
283 个 TMTR 中有两个非零条件块指针（ActionTimeline 11545、12433），各三项；
验证脚本也比较这六项的操作码、整数参数与浮点位。条件布局命名另参照同版本
Actor/Tmac、Track/Tmtr 与 TmtrLuaEntry 的字段顺序。飞石、其子时间线与
`ability/thm_black/abl001` 对应的三份 AVFX 共引用 15 个不同纹理路径；
逐一定位并由 `validate_tex` 在 CPU 解码，共 118 个 mip、118 个 image。
命令字段命名参考 Dalamud-VFXEditor `ed6b39dcca04225c15c404a35a46945119462015`：
先核对根 LICENSE（MIT，Copyright 2021 Michael Kaminsky），后查看 TmbFormat 的
TmbItem、C002/009/010/012/063 字段声明/顺序、绑定枚举和角度类型，以及 AvfxFormat
的 KeyType 与编辑器曲线显示分支。Rust 标量插值以客户端指令为准；没有移植编辑器播放代码。

`normal/idle.tmb` SHA-256 `9ddbbb05e62408df183471acecfa84979e21aadd5e7d810dae9fed3f1fb3a5ec`。
c0101 base SKLB 为 `a01a5c20a7d2dfd5cf19c03c94e483bce3840ada0614018a717d22ad97240820`。
`verify_action_pose.py` 通过独立 tagfile 观察器、Cox–de Boor 和 4×4 矩阵连乘核对
`action_pose` 的 `n_buki_r` 骨架空间帧；0、125、500、1000、2500 毫秒共 60 个
分量，最大绝对差小于 `1.1e-7`。这是普通 clip 的数学结果，不验证加法层或角色与武器的持握校正。

```sh
cargo build --examples --all-features
python tools/verify_action_effects.py ActionTimeline_0.exd extracted-by-row-id target/debug/examples/inspect_motion
python tools/verify_action_pose.py idle.tmb bt_common_resident-idle.pap base.sklb target/debug/examples/action_pose
```
