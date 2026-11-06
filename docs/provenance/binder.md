# AVFX Binder 与武器宿主元数据证据

## 来源与许可

需求来自 xiv-companion 的 `docs/render-book/vfx-assets.not`：binder 的 BPID 与
实际 owner MDL ElementId / SKLB 关联。只读需求文档，不读取该项目解析代码或
本项目 legacy，不实现角色装配、GPU、运行时宿主选择或特效模拟。

对已使用的 Dalamud-VFXEditor commit `ed6b39dcca04225c15c404a35a46945119462015`
先重新获取根 LICENSE，确认为 MIT、Copyright 2021 Michael Kaminsky，再阅读：

- `VFXEditor/Formats/AvfxFormat/Binder/AvfxBinderProperties.cs` 截止构造方法之前
  的声明，包含字段、UI 成员及静态 BinderIds 名称字典；
- `Binder/AvfxBinderPropertiesName.cs` 全文，包含 Name 字符串的空负载与零填充读写；
- `Base/Enums.cs` 的 BinderRotation、BinderType、BindPoint、BindTargetPoint 声明。

源中的 BinderIds UI 字典确已见到，但不被复制为全局 ID→骨名或固定变换表，
也不作为运行时优先级的证据。Rust 使用独立的受检块视图和显式资源预算，
Python 用 struct 独立观察；没有复制、翻译或机械移植来源解析实现，不引入其
依赖，不宣称严格 clean-room。

## 属性块样本

Win32 安装 `2026.10.06.0000.0000`。基础六份与扩展二十份 AVFX 的 binder 属性
逐项对照共 40 槽：34 Start、4 Goal、1 First、1 Second。BPT/BPTP 组合包括
`0/0,0/1,0/3,1/0,1/2,1/3`。名字均为字面量 `null`，有零填充，不能据此把
所有非空 Name 视作缺省名称；空与缺少 Name 的区别另有受控回归。

`tools/verify_avfx_assets.py` 同时核对 `examples/validate_binder.rs` 的全部选择器、
名字节、延迟/更新/坐标参数、ring 标量、位置组边界。属性行哈希分别固定在
`tests/fixtures/avfx{,-variant}-observations.json`。没有属性槽的样本输出空列表，
不是创建默认 Start 或 Goal。

## 武器资源链

`tools/find_weapon_vfx.py` 在 base chara index 对 weapon roots w0101..w2901、
body 1..2000 的最多 58000 个命名候选作有界查找。检查 21 个候选，其中 20 个
IMC 可定位、3 个有非零 VFX ID，取得以下路径链的实际资源：

| Weapon / body | IMC 非零 subset | VfxId | AVFX 文件名 | Binder BPID |
| --- | --- | --- | --- | --- |
| w0101 / b0011 | 2 | 1 | vw0001.avfx | 5、3、4、4 |
| w0101 / b0019 | 2 | 2 | vw0002.avfx | 3 |
| w0101 / b0021 | 2 | 2 | vw0002.avfx | 4 |

完整路径和资源哈希位于 `tests/fixtures/weapon-binder-observations.json`。
例如第一条为 `chara/weapon/w0101/obj/body/b0011/b0011.imc` → 同 body 下
`vfx/eff/vw0001.avfx` 与 `model/w0101b0011.mdl`，骨架为
`chara/weapon/w0101/skeleton/base/b0001/skl_w0101b0001.sklb`。
候选命名通过 index 定位、资源头和字段关联逐项验证，不推广为所有武器领域、
主副件共享或路径回退的运行时规则。

三份 IMC 的全部记录分别与 Rust 输出比较，VfxId 1/2 的非零位置均为 subset 2。
AVFX / SKLB 的 type-2 解压与 MDL 的 type-3 全逻辑重建各有独立 Python 字节
对照。三份 MDL 合计 3733 顶点，完整 scalar/index 输出再与独立 MDL observer
比较；SKLB v1200 的完整骨架元数据和 rest TRS 也独立逐字段核对。

六个实际选择项均为 BPT=0、BPTP=3、Name=`null`。每个 BPID 都在实际 owner
MDL 中唯一匹配一条 ElementId，其父骨名又唯一匹配 animation container 所选
骨架的一条骨。匹配为 `n_hara`，对象 ID 3、bone index 1，所选骨架共两骨。
MDL 的 ID 3/4/5 是标识，不等于骨索引。ElementId 位移由每个 owner 原值输出；
没有根据 source UI 字典或其他模型的骨顺序重排。

该链是静态一致性证据，不证明客户端执行时一定采用这条候选、Name/BPID 的
优先级、Caster/Target 生命周期、曲线驱动或局部到世界矩阵的合成。

## 受控回归与复现

回归覆盖缺省槽、空 Name 与缺少 Name、字面量 null、CoUF=-1、负 BPID、
负零浮点、未知属性、重复字段/槽、非终止字符串、错误宽度、预算及 4096 次
确定性变异。未知目标值保持原整数，不被转为一个默认的已解析骨骼。

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_binder \
  --example validate_imc --example validate_mdl --example validate_skeleton
python tools/verify_weapon_binder.py /path/to/game/sqpack /tmp/opencode/weapon-binder-check
```

完整报告、提取文件与输出行写入新目录，不修改安装或将游戏内容放入仓库。
