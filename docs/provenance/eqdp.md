# EQDP 观察与验证

## 来源

稀疏块布局的初次观察仅使用用户授权的 Win32 安装 `2026.10.06.0000.0000` 二进制和本库已有 SqPack
提取器，未读取 legacy。Lumina 与 VFXEditor 的 GitHub
文件路径清单搜索没有返回 EQDP 文件，因此没有取得或采用其 EQDP 代码。
文件布局由 Python struct 直接检查，Rust 借用表独立实现随机访问；不作严格
clean-room 声明。专有原始样本和展开数据不入库。

在 CMP/EQP 字段审查中又接触 TexTools/xivModdingFramework 的
`Models/DataContainers/EquipmentParameter.cs`，commit、GPL-3.0 许可和实际阅读
范围见 [EQP](eqp.md)。该文件也包含 EquipmentDeformationParameter 的 low material /
high model 声明及方法，并注明 material 位可能不被游戏使用；没有采用其 BitArray 实现。
这补充公开命名来源，不替代缺失的游戏运行时位义证据。

## 结构证据

`chara/xls/charadb/equipmentdeformerparameter/c0101.eqdp`、`c0201.eqdp`
均为 8192 字节，头为 `01 01 40 00 9d 00`。块指针包含 0、64、128 等偏移，
以及穿插的 `0xffff`；以 u16 条目为单位寻址后，全部引用落在载荷内。
`tools/verify_eqdp.py` 顺序展开所有块，与 Rust 的每条随机访问结果逐行核对，
包含缺省块来源标记。哈希固定在 `tests/fixtures/eqdp-observations.json`。

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_eqdp
python tools/verify_eqdp.py /path/to/game/sqpack /tmp/opencode/eqdp-check
```

## 位义推断和限制

独立以 SqPack index 哈希检查对应 race/equipment/slot 的 MDL 路径：c0101 两位值
为 1 的 101 个槽位均无同编号 MDL；值为 2 的 100 个中有 99 个有同编号 MDL。
c0201 值为 2 的 3328 个中 3129 个有同编号 MDL，值为 1 的两个均无该 MDL。
这支持高位与模型声明相关，但不能证明所有编号都存在直接模型。

材质路径以 `material/v0001..v0003/mt_c####e####_{slot}_a.mtrl` 探测：
c0101 值为 1 的 101 项中 58 项命中，值为 2 的 100 项中也有 25 项命中。
材质与模型存在回退、别名和版本目录，直接存在性不足以独立证实低位完整语义。
因此 material/model 命名标记为推断；低位的完整运行时语义仍需额外证据。

槽位探测使用 head/body/hands/legs/feet 对应路径后缀 met/top/glv/dwn/sho，
只读 index 表，不启动游戏。猜测的 `c0101_acc.eqdp` 路径返回确认不存在；
这不证明安装中没有其他配饰命名变体。

## 配饰目录、稀疏变体与槽位关联

同一授权 Win32 安装中，对候选文件名/目录作 index 哈希查找，
`equipmentdeformerparameter/c####a.eqdp` 没有命中，但
`accessorydeformerparameter/c####.eqdp` 有实际记录。这里不咨询额外第三方源码。
命中后以 Rust SqPack 提取和独立 Python struct/zlib type-2 解压逐字节对照。

扩展样本为配饰 c0101/c0201/c0401/c0801/c1701/c1801 和装备 c1701/c1801，
完整身份与行哈希固定在 `tests/fixtures/eqdp-variant-observations.json`。
八份各有 10048 逻辑条目，合计 80384；连同基础两表共 100480 条目。
配饰文件大小分别为 2048/1024/512/1024/1024/1024 字节；它们与装备表共用
0x0101、B=64、N=157，但缺省块数和尾部填充不同。c0401 的全部 157 块均为
`0xffff`，其 10048 条目均为隐式零；这不表示该 race 没有可通过其他路径使用的配饰。

每份表同时对照原始随机访问结果与有领域名的五槽 API 输出；所有原始 word、
缺省块来源、五槽两位值一致。领域由文件路径明确提供，parser 不从 word 猜测。

`tools/verify_eqdp_links.py` 对六个 race 的全部 10048×5 槽位查询同名 MDL
是否出现在 base chara index，按槽位和值 0/1/2/3 记录命中与缺失。再选 c0101/
c0201 的每槽首个声明高位且可定位的模型，共十份，独立重建完整 type-3 MDL，
并把所有元数据、顶点与索引输出与独立 observer 比较：合计 6132 顶点。
骨名证据包括耳 `n_ear_a_l/r`、neck `j_sebo_c/j_kubi`、wrist 手腕/前臂骨，
ring 分别 `j_hito_a_r` 与 `j_hito_a_l`。模型命名、实际 EQDP 槽和 MDL 骨名
相互支持 ear/nek/wrs/rir/ril 的顺序，不只凭一份自行构造的位域测试。

十份 MDL 的全部材质字符串也独立比较，其中：

- c0101/a0001 左 ring 槽为 2，实际左模型材质是右 ring 的
  `/mt_c0101a0001_rir_a.mtrl`；
- c0201 的五个模型槽均选到值 2 的实例，其材质字符串均引用 c0101 命名，
  左 ring 同样引用 rir 命名材质；
- c0101/a0139 和 a0157 耳饰槽为 1，同 race 同 ID MDL 在 base index 中缺失，
  v0001 的对应材质存在；两份 MTRL 的全部 keys、constants、samplers、纹理名、
  颜色表均与独立 struct observer 逐行一致。

这些静态记录支持两个独立声明位及材质共享，不证明完整客户端路由或 material
位在所有加载路径上实际被消费。路径缺失只指这次查询的完整路径和 base index，
不推广为资源全局不存在。所有关联记录、hash 和全槽统计的报告 SHA-256 固定为
fixture 的 `links_sha256`；报告和游戏字节不入库。

```sh
nix develop -c cargo build --features local --example local_sqpack \
  --example validate_eqdp --example validate_mdl --example validate_materials
python tools/verify_eqdp.py /path/to/game/sqpack /tmp/opencode/eqdp-variants-check \
  tests/fixtures/eqdp-variant-observations.json
python tools/verify_eqdp_links.py /path/to/game/sqpack /tmp/opencode/eqdp-links-check
```
