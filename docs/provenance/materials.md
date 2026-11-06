# MTRL 与 SHPK 来源和验证

16-half 源行到 32-half 中间行的逐 lane 数据流、首行扩展、全 half 域
数值契约及完整表输出见 [Legacy 色行转换](legacy-color-conversion.md)。
其中双通道、profile、reset 与缺模板数据关系有静态依据，5620 个完整
表请求具有独立输出对照；运行期上传和真实 544 字节资源的边界分别列出。

## 染色覆盖 profile 与中间行

### 静态观察

固定 Win64 executable 身份、授权和 GNU objdump 2.46 的边界沿用本页。
离线阅读 `14042b730..14042b960`、`141883cf0..141883ff0`、
`14042ae50..14042b25d`、`14042b430..14042b4c0` 与
`140426c20..140426ca0`；没有运行、附加或模拟客户端，没有读取额外第三方
实现。库按已记录字段关系编写固定大小的转换与染色操作；没有移植客户端
的资源缓存、分配或指令控制流。公开搜索片段接触见 Legacy 色行转换记录。

- `140426c63` 引用 `shader/sm5/shpk/characterlegacy.shpk`，对应资源请求
  的结果存入 utility `+0x2b8`。`14042b750/14042b777` 将材质 `+0xc8`
  的 shader 身份与这个指针比较。相等时选择五项表，否则选择十二项表；
  此处不按 MTRL 行数选择。两表位于 RVA `0x28aaa40/0x28aa9e0`。
- 五项表的 `(flag,目标lane)` 为 `[(0,0),(1,4),(2,8),(3,3),(4,7)]`；
  十二项表为 `[(0,0),(1,4),(2,8),(3,11),(4,18),(5,16),(6,12),
  (7,13),(8,14),(9,19),(10,27),(11,21)]`。这些目标属于 32-half
  中间行。位号和 half 下标是数据格式关联，不复制该表驱动消费过程。
- `14042aef0..14042b06e` 显示 Legacy 色行进入转换；例如源 lane 3
  通过数值转换后参与中间 lane 7 的生成，多个目标还由常量产生。
  `14042b430..14042b494` 的另一分支按源行跨度复制到 64 字节目标行。
  文件字段名不能仅由转换后的目标下标反推。完整转换数值定义与 Rust
  输出对照见 Legacy 色行转换记录。
- `141883d40..141883e3b` 的 STM 列读取助手按列编号小于 3／其余选择
  分量数，RVA `0x22dcfd0` 的两个 u64 常量是 `[3,1]`。调用点分别位于
  `14042b891/14042b949`；表中的前三项为 RGB，其余是 scalar。
- `14042b7fd` 将 dye 行的 channel 与请求值比较，不匹配则跳过该行。
  specular RGB 经临时缓冲区读取后，`14042b896..14042b8bf` 仅在三个
  half 原始位不全为零时写回。三个 `0x0000` 保留原分量；这是位判断，
  不含 `0x8000`。模板缺失与 Stain 范围还可影响结果，覆盖掩码本身不
  等于最终变更掩码。

`client-mtrl-observations.json` 固定四段额外区间和九个额外锚点，复验共
十段静态范围；工具还核对 shader 路径 LEA、两个覆盖表引用、宽度常量引用
及两个直接调用目标。报告是普通静态证据，不称为 REA Evidence；实际材质
是否在某次运行走该分支没有执行验证。

### 独立声明投影与实文件

`DyeRow::canonical_override_lane_mask(DyeOverrideProfile)` 由本项目编写，
只对数值 flags 作独立集合投影，不执行 native 条件树。调用者必须显式
选择 Legacy 或 Dawntrail 目标 profile。源 `ColorLayout` 只决定 dye 字的
flags 范围；模板、channel 与未知位不扩展该范围。输出的 bit n 表示中间
行 lane n 可被替换，不声称已写入，也不是 Legacy 文件行上的写掩码。

`verify_dye_lanes.py` 在验证 executable 摘要后直接读取两张静态字段表和
`[3,1]` 元数据，Python 按 lane 集合求并集，再与 Rust 输出逐行比较。
controlled 输入包括 Dawntrail 全部 4096 个 flags 子集，各搭配零高位、
全部未知位、最大模板/通道三个变体，共 12288 个 word；Legacy 则遍历
全部 65536 个 u16。每个 word 均比较两个显式目标 profile，共 155648 次
受控投影。Legacy 模板位不会成为额外 flags；两种 profile 的相同低位也
不会被错误地视作相同目标。

实际输入使用本页 M0 武器与 M4 装备材质：SHA-256 分别为
`1148dd61091b3c12eeaf441fb6cc69383b3e06c6c1768c111ef1946a9832504a`、
`2bade0293506117449ac4424e6662dfe3d00bb652f75fa48cc045bc2ab3cfb93`。
二者均有 32 行 dye 数据，但 M4 的 shader 为 `characterlegacy.shpk`。
64 个实际 word 的两 profile 投影共 128 次比较，原始 dye 字由 Python
独立按 MTRL 区段位置取得；没有通过运行客户端获取最终颜色。

`dye-lane-observations.json` 固定来源及报告 SHA-256
`7653f24979f64b2d7f72b1c3a91ab8a7987de29adad5df01d899287cf6b21ed9`；
完整行摘要为 `b06c3ce6508ba6d70c605ba0fad4e584675c6d7737b85940202fa2b498255f6b`。
单元回归覆盖 profile 与源布局独立、每个单置位、无 flags、全 flags、
非零未知位及 Legacy 模板位隔离。资源、全部行与反汇编留在仓库外。

声明投影的验收对象是覆盖目标、profile 选择依据和局部条件。完整色表
求值另有[数值回归](legacy-color-conversion.md#完整色表输出与染色关联)；
两者均不代替 shader 像素效果或实际 544 字节 Legacy 染色材质的见证。

### 显式 half 行覆盖的独立数值对照

列与目标的关联除静态字段表外，还核对 `14042b844` 的 flags 位号进入
`r8d`，并在 `14042b891/14042b949` 调用列助手；`141883d73` 按该编号
区分前三列，`141883d94..141883d99` 用它读取累积结束偏移。它说明 STM
列编号与覆盖位号的关联；不据此复制模板缓存、资源选择、回退或客户端
整段控制流。该观察使用上述相同固定二进制和已有摘要区间，没有执行目标。

`mtrl::dye` 由本项目按字段关联编写独立的不可变行替换操作。源为调用者
提供的 32 个 half 位模式，模板由 dye ID 精确查询，profile 与 STM 布局
必须相符。结果检查覆盖实际行输出与写入掩码。
源表布局与目标 profile 分别选择；Legacy 文件行的零扩展不冒充颜色转换。
缺模板和越界 stain 为显式错误；空掩码也检查输入，错误不提供猜测行。
通道门控的 mismatch 返回无结果，Legacy 无通道字要求显式选择行。
这种 API 错误策略不声称复现客户端的缺省/回退策略。

`verify_dye_rows.py` 独立读取两份实际 STM 的累积结束表和 half 列，直接
从指定 executable 的两张格式表取得目的 lane，再按区间替换计算输出。
Rust 示例消费独立生成的二进制请求，比较每个输出的全部 32 个 half、
写入掩码和结果类别。材料来源为上述 M0/M4 的 64 条真实 32-half 行。
每条行与两种显式 profile 的 253 个 stain 共 32384 个请求，另有匹配/
不匹配 channel 请求；其中 2286 个返回行、128 个通道跳过、30226 个
缺模板错误。profile 两两对照包含有意不匹配的模板 ID；缺模板错误不算
成功染色，也不把这组 profile 请求视为 native shader 选择观察。

受控部分使用全部 86 个实际模板及其 253 个有效 stain、两种 profile 各
全部 4096 个 flags 子集，另检查 stain 0/254/255/65535、缺模板、Legacy
无 channel 和无效 channel。这些声明及位丰富的源行是受控输入，STM
数值来自实际文件。总计 62624 个请求：32246 个行结果、128 个跳过、
30228 个缺模板、16 个非法 stain、2 个无编码 channel、4 个无效 channel。
6530 次 specular 全部原始位零时保留源值；每个结果的未选择 lane 逐位对照。
`tests/mtrl_dye.rs` 另覆盖负零、half NaN payload、非零 subnormal、源布局
与目标 profile 分离、Legacy 未知高位不影响 template、错误不修改源行。

固定报告摘要见 `tests/fixtures/dye-row-observations.json`：
`945ef94f030c22edd43bc590a1378bc0a04697814afdd54cba200491c5e4568e`。
请求摘要为 `6b225fd0fada7e46be71dd27f9e866eec5ea4fedb3281ad7a205228160e61ff7`，
结果摘要为 `2d71dc71289a30cfc7abd58f99dd634d73d69a7c721ef0a152840012bc977a57`。
全部原始行、生成请求和两侧输出留在仓库外；本对照不验证 Legacy 色转换、
stain-zero/reset、缺省模板策略、多个请求的先后顺序或完整材质播放。

```sh
python tools/verify_dye_rows.py /path/to/ffxiv_dx11.exe \
  /path/to/legacy.stm /path/to/dawntrail.stm \
  /path/to/weapon.mtrl /path/to/equipment.mtrl /tmp/opencode/dye-row-operations
```

```sh
nix develop -c cargo build --features local --examples
python tools/verify_dye_lanes.py /path/to/ffxiv_dx11.exe \
  /path/to/weapon.mtrl /path/to/equipment.mtrl /tmp/opencode/dye-lane-profiles
```

## 许可审查与阅读范围

需求来自 xiv-companion commit `a38d3c9bfb67fb441d17a77b344f77c83058dca0` 的 `docs/render-book/materials.not`。
未读取该项目的解析实现或其第三方代码。

- Lumina commit `8802d66a3cb3032be2fcf998bb10e864221bdb13`，WTFPL v2，许可证先于阅读审查，见 [IMC 记录](imc.md)。
  阅读 `Data/Parsing/MtrlStructs.cs` 全部 131 行，以及 `Data/Files/MtrlFile.cs` 的 `LoadFile` 之前的数据成员。
  字段声明提供 16 字节头、描述表和旧颜色表的公开命名。
- [0ceal0t/Dalamud-VFXEditor](https://github.com/0ceal0t/Dalamud-VFXEditor) commit
  `ed6b39dcca04225c15c404a35a46945119462015`。
  阅读内容前取得仓库 `LICENSE`：MIT，Copyright 2021 Michael Kaminsky。
  审阅 `Formats/MtrlFormat/MtrlFile.cs`、`Data/Color/{MtrlColorRow,MtrlColorRowLegacy,ParsedTileMaterial,ParsedHalfShort}.cs`、
  `Data/Dye/{MtrlDyeRow,MtrlDyeRowLegacy}.cs`；
  `Formats/ShpkFormat/ShpkFile.cs`、`Shaders/{ShpkShader,ShpkParameterInfo}.cs`、
  `Keys/ShpkKey.cs`、`Nodes/{ShpkNode,ShpkNodeKey,SphkPass}.cs`。
  阅读包含构造、读取、写入和部分 UI 方法；未调用其中 shader 编译、运行、反汇编功能。
  `ParsedSphereMaterial.cs` 的请求连接失败，未作为已读证据。
- Ottermandias/Penumbra.GameData 元数据 `license: null`，根目录列表没有 LICENSE。
  未读取其解析代码。VFXEditor 部分文件注释引用该项目，此上游来源限制一并保留；
  不将顶层 MIT 声明视为未知上游代码的再许可，也不采用其代码作为本库实现。
- `xivdev/Penumbra` 的许可证元数据为空，`NotNite/ffxiv-shaders` 返回 404；均未读取解析内容。
  SaintCoinach 元数据为 WTFPL，未阅读其内容或采用其结论。

本库的受检借用表、索引预算、scalar 访问和资源边界由本项目编写，没有复制、翻译或照搬解析代码，
没有引入任何现成 FFXIV 解析库。第三方公开字段结论均与下述真实资源观察区分记录。
审阅接触如上，不宣称严格 clean-room；未读取 `legacy/`。

## 样本身份

来源：用户授权的只读 `/home/azurice/Games/ffxiv/game/sqpack`，Win32，
`ffxivgame.ver = 2026.10.06.0000.0000`。下表 MTRL version 为 `0x01030000`；七份 SHPK 为 `0x0e01`、DX11。
样本不提交或再分发；`tests/fixtures/material-observations.json` 保存文件和语义行的 SHA-256。

### MTRL

| 编号 | 游戏路径或索引身份 | 字节数 / dataset | SHA-256 |
| --- | --- | --- | --- |
| M0 | `chara/weapon/w2001/obj/body/b0102/material/v0001/mt_w2001b0102_a.mtrl` | 2680 / 2176 | `1148dd61091b3c12eeaf441fb6cc69383b3e06c6c1768c111ef1946a9832504a` |
| M1 | `040000.win32.index` 序号 779，hash `0075d1a8461898f7`；dat0 偏移 1569636480 | 932 / 512 | `5f1b027bf4e8b3d4a93f0c2a5aba7f5d6573d4e368b8d20ad7e38d2ed64f557e` |
| M2 | `chara/human/c0101/obj/face/f0001/material/mt_c0101f0001_fac_a.mtrl` | 512 / 0 | `ba9a2e4edc40841b4b9bb883a129c09d2ceb697616ff829d761c272179608cf1` |
| M3 | `bgcommon/hou/dyna/opt/sg/0009/material/opt_sg_m0009_3a.mtrl` | 744 / 0 | `f974a131c29f544150bde582a13f455870a5de45f3c547e871197d3f453a876f` |
| M4 | `chara/equipment/e0001/material/v0001/mt_c0101e0001_top_a.mtrl` | 2640 / 2176 | `2bade0293506117449ac4424e6662dfe3d00bb652f75fa48cc045bc2ab3cfb93` |

M1 来自基础包 `ffxiv/040000.win32.dat0`，未恢复其原始资源路径；没有将纹理路径误作 MTRL 路径。
M0、M2、M3、M4 的引用来自已验证 MDL 的材质名及其实际资源目录。

### SHPK

所有路径位于 `shader/sm5/shpk/`：

| 文件 | 字节数 | Vertex / Pixel | SHA-256 |
| --- | --- | --- | --- |
| `character.shpk` | 26099125 | 176 / 1038 | `c2453552ce2527f1f92a9f62d04d2c5fbed67226306629826edc0c1d9c646abe` |
| `characterlegacy.shpk` | 39030446 | 64 / 1740 | `5f95ae36f1cfbf603f85817d6872190a56ddd323fb44ade17779bbaf3a28049e` |
| `skin.shpk` | 11733754 | 72 / 384 | `4e2f77df0399de59af5e6383358cb7d071ca0c7734f2e161504b086239593e57` |
| `bgcrestchange.shpk` | 168369 | 9 / 12 | `fa60e4f6a706c5ee0ff9190d9740248d8fc1802df0bfcb572049a6af5e6ad3d6` |
| `water.shpk` | 14379730 | 19 / 777 | `40b749201ceea5332d60500ea37df61bae43372f8db55f23982ac5c5dbfe69fd` |
| `bg.shpk` | 90101224 | 56 / 3850 | `9617282ff8a947ceecd11846a73fc1d5ec7782cf19fe22ec363cc6d3195375be` |
| `hair.shpk` | 10023788 | 192 / 200 | `ed0c9953e6aa6db7822a205e051c0628e2ad31bd87fb38e6b28ff042570a015f` |

这七份样本的 hull/domain/geometry count 均为零；含 Geometry 的独立样本与
0x0d01 变体见下述库存与验证记录，不把受控五阶段测试描述为五阶段真实验证。
`bg.shpk` 的 90101224 字节超过示例默认 64 MiB，默认预算明确返回 ReadTooLarge；
使用显式 128 MiB 预算后正常提取，未取消预算检查。

## 观察与结论

### MTRL 布局与语义关联

- 声明文件长度、字符串长度、additional/dataset 分区、常量和 sampler 表全部与实测文件长度吻合。
  M3 的 additional data 为 36 字节，其余为 4 字节；不固定跳过四字节。
- M1 为 16 × 16 half，M0/M4 为 32 × 32 half；M0/M4 紧随 32 个 u32 dye 行。
  Python `struct` 独立解码每个 half 与 raw dye 位，全部输出行与 Rust 相等。
  三份有表材质合计 80 个颜色行，其中 251 个 half 值大于 1，未被限幅。
- M1 和 M4 都引用 `characterlegacy.shpk`，却分别使用 16 行与 32 行表，不能由 shader 名推断布局。
- M2/M3 有 color-set 名称而 dataset 长度为零；颜色表 getter 正常返回缺省状态。
  未识别的非空 dataset 在 API 中为明确不支持错误，而非 None。
- M1 的四个 sampler 对应三个 texture 路径，含 `0xff` texture index；原始索引被保留。
- 五份材质的 9 个 key、60 个 constant、18 个 sampler 的 ID 全部可在对应 SHPK 中解析。
  这验证 ID、字节范围和关联，不声称已经验证游戏 shader 的全部公式。
- 公开来源给出的 diffuse/specular/emissive、粗糙度、sheen、tile/sphere 字段命名与 half 编码分开记录：
  编码和数值已由样本验证，字段的完整游戏求值与染色属性位义需 STM/关联实验补充。
  Dawntrail 模板提取值包括 200、1200、1202、1210、1212、1500；模板真实存在性需 STM 验证。

### SHPK 引用与布局校正

- 7 个包共 8589 个 shader，均验证 DXBC magic、内部长度及所引用的完整字节。
  Vertex 的 8 字节前缀与其他实测阶段的零前缀分别验证。
- 210453 个全局/每-shader 资源名称引用的终止符、长度、slot/size 及名称 CRC 验证通过。
  CRC 用独立 `~zlib.crc32(name, 0xffffffff)` 对照，并确认它不同于 SqPack JAMCRC。
- 合计 492 个 material parameter 范围、29049 个 node、132234 个 pass、594055 个 alias 均有效。
  五份材质到四类 package 的 ID 关联全部成立。
- 来源 `ShpkNode.cs` 把两项未知 u32 放在 pass 表后。真实 `character.shpk` 首 node 若按此读取，
  首 pass 的 vertex index 为 2578591004，超过 176 个 vertex shader，属于失败假设。
  真实数据中在固定 pass-index 数组后先有两项 u32，再有 key values，最后为 24 字节 pass。
  按这一布局，全部七份包的 shader index、node 长度与 alias 起点相互吻合。
  此校正来自独立观察，没有机械翻译该公开读取方法。
- node-alias cluster 只保留边界内的完整原始字节及声明数量，不伪造层级和 permutation 语义。
  元数据默认值读取不依赖执行 shader 或求解 cluster。

## Legacy 染色样本缺口与尝试

`tools/find_material_variants.py` 对基础 chara 和 bgcommon 的各前 100000 条 index 记录，
检查长度不超过 8192 字节的普通条目及其首块。
544 字节 dataset 的扫描检查 72888 个小条目，观察到 24304 个 MTRL，匹配数为零。
这只是有界负向证据，不能推断安装内不存在该变体。
安装 `update_pack/` 的两个子目录内没有发现文件。
544 字节 Legacy dye 的实际游戏资源样本未在该范围发现；其布局与位域的独立
原生静态观察及全部 u16 的受控对照见下文，不把受控文件称为游戏实文件。

## MTRL 声明与 Legacy 染色的原生静态证据

目标为授权安装 Win64 executable，51910400 字节，SHA-256
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`，首选
image base `0x140000000`。以下为 GNU objdump 2.46 离线指令观察，不启动、
附加或模拟目标，不是 REA Evidence。原生引擎能力限制与替代路径见来源总表。
没有读取新增第三方解析实现、移植 native 颜色转换或执行染色算法。

- `14031b509..14031b53b` 登记 mtrl resource 类型；
  `140301290..140301396` 校验 `0x01030000`/长度、读取 16 字节头，建立
  additional-data `+0xe8`、dataset `+0xf0` 的对象指针。实例偏移是该程序
  的实现事实，不写进文件格式布局。
- `14042adf9..14042ae79`：内嵌表路径读取 additional-data 的低位宽高字段。
  bits 4–7 为 half4 宽度指数，8–11 为行数指数；宽度指数非零时使用幂次，
  色区跨度按 half4 宽度 × 行数 × 8 字节计算。宽度指数零使用 4 组 half4、
  16 行，染色起点为 dataset `+0x200`。实际 M1 的首 u32 为 `0x5`，
  M0/M4 为 `0x53c`，与 16×16 / 32×32 half 的实际长度独立一致。
- `14042b1bf..14042b42b` 从该 Legacy 染色起点读取 `+0,+2,…,+30` 的
  16 个 u16；每条取低五位并右移五位分离剩余十一位，构成供消费的规范记录。
  由此独立确认 512 字节颜色区之后是 32 字节 dye、五位覆盖与十一位模板，
  不依赖按同样假设手写的合成测试。
- `140426e15..140426e6a` 取得两份 stainingtemplate 资源；
  `14042b730..14042b953` 按 shader 身份选择 STM，以十一位索引查模板，
  使用覆盖位与选择通道处理规范化的 32 条记录。这里没有声称 Legacy 文件
  原始 dye 为 32 个 u32；Legacy 的 16 个 u16 与消费阶段记录分开处理。

消费点的两个 `(override bit, canonical half-lane)` 表也已固定：五项表为
`(0,0),(1,4),(2,8),(3,3),(4,7)`；十二项表为
`(0,0),(1,4),(2,8),(3,11),(4,18),(5,16),(6,12),(7,13),(8,14),
(9,19),(10,27),(11,21)`。这些 lane 指向转换后的表，不直接冒充 Legacy
原始行坐标或完整属性命名证明；Legacy 色行转换、模板优先级及 shader 公式
没有机械移植到库。字段数值语义与材质最终外观分别验收。

宽高字段说明了仅看总长度的歧义：受控 `32 half × 8 rows + 8 u32 dye`
也为 544 字节，不能误称为 `16×16 half + 16 u16 dye`。库提供
`inline_color_table_declaration()` 的原始 flags/half-lanes/rows/dye 视图；
固定 16/32 行颜色表 getter 在存在声明时核对尺寸与 dye 位，未支持的声明
返回明确布局错误，原始 dataset 仍可访问。不通过长度匹配忽略显式声明。
受控回归还覆盖最大四位指数、dye 位与长度冲突、缺少内嵌声明与原有原始视图。

`verify_client_mtrl.py` 校验六段区间摘要、字段锚点、全部 u16 读取位置与两个
消费表。以固定实际 M1 颜色数据为基础，在仓库外插入 32 字节 dye、调整
文件长度/dataset 长度及染色位，建立 964 字节受控 MTRL。该文件所有字段与
Python struct 独立观察一致；`validate_legacy_dye` 再以十六行一页覆盖全部
65536 个 u16 值，flags/template/未知位/无通道输出与 Python divmod 完全一致，
每页均确认原始颜色行不变。没有将模板号合法存在性、染剂应用或 shader 输出
作为这个代数分解实验的结论。

源文件、受控文件、原生区间与完整行摘要固定在
`tests/fixtures/client-mtrl-observations.json`；源字节、受控字节及反汇编留在
外部报告目录。完整回归另检查真实 MTRL 的声明尺寸与各色表长度。

```sh
nix develop -c cargo build --example validate_legacy_dye --example validate_materials
python tools/verify_client_mtrl.py /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/material-1.mtrl /tmp/opencode/client-mtrl-check
```

## 角色主索引的 Legacy 材质调查

`find_legacy_dye_materials.py` 只读授权的 Win32 安装
`2026.10.06.0000.0000`，在角色索引中寻找 dataset 长度为 544 的 Legacy
染色材质。工具的选择预算为 500000 条记录、每索引表 64 MiB、每候选解码
65535 字节，最多接收四份染色材质；65535 来自 MTRL 的 u16 文件长度。
先核对 type-2 头与首块 raw/DEFLATE 长度，再筛选 `0x01030000` 及匹配的
文件长度。首块签名与 dataset 长度统计是候选观察，不等同于完整材质验证。
没有读取第三方实现或本项目归档代码。

安装中匹配 `*/04*.win32.index` 的文件只有
`ffxiv/040000.win32.index`，其 334633 条记录全部被选择。表 SHA-256 为
`3570cbbd52654e524629b3e083669b6d844e5c251a88a9c655a988700abff051`。
196873 个符合大小界限的 type-2 首块接受检查。匹配 MTRL 版本/文件长度的
头部中，dataset 长度候选分布为：0 字节 2572 份、512 字节 322 份、2048
字节 26394 份、2176 字节 13153 份，没有 544 字节候选。结论限于该索引
选择和筛选条件，不代表其他类别、未列入的安装包或历史版本不存在 Legacy dye。

工具另对全部 322 份 512 字节 dataset 材质完整解压，以 Python struct
独立解释每个描述项、字符串、key、constant、sampler、additional data 和
全部色行。Rust `validate_materials` 的完整语义行与 Python 输出逐字节一致，
每份原始文件和语义行均记录 SHA-256，不只比较尺寸或样本数量。颜色声明
存在时另核对其 half-lanes、行数及染色标志与负载的关系。

这批实际材质含 5152 行、82432 个 half，其中 15543 个值大于 1，没有负值。
关联 shader 为 `characterlegacy.shpk` 197 份、`character.shpk` 67 份、
`skin.shpk` 27 份、`characterreflection.shpk` 23 份、
`characterocclusion.shpk` 6 份、`hair.shpk` 1 份、`charactertattoo.shpk` 1 份。
该观察再次说明 shader 名称不能单独用来选择颜色表的物理行布局；不由这些
文件推断 shader 的最终颜色公式，也不把无染色的文件算作实际染色样本。

完整报告 SHA-256 为
`5895cfd295b30748d8b3719e4fb82660367661ca99729180871295055fd8d430`。
`tests/fixtures/legacy-mtrl-discovery-observations.json` 固定报告摘要、选择边界及
正向验证计数；资源字节、两侧逐字段输出和包含每份材质身份的报告留在仓库外。
普通资源联跑将此任务列为 inventory observation：报告同时包含有界的负向
搜索与 322 份正向完整材质对照，不因此增加实际 Legacy dye 的验收计数。

```sh
nix develop -c cargo build --example validate_materials
python tools/find_legacy_dye_materials.py /path/to/game/sqpack \
  /tmp/opencode/legacy-mtrl-inventory
```

解除这一局部验证缺口需要可定位的 Legacy 16 行带 dye MTRL，或获许可且可独立验证的格式证据/样本。
在其他允许的资源搜索与格式工作仍可推进时，此缺口不作为整体任务的停止理由。

## 复现

在根目录构建示例。输出文件采用 create-new，不覆盖；安装始终只读。

```sh
nix develop -c cargo build --features local --example local_sqpack --example extract_entry --example validate_materials
target/debug/examples/local_sqpack /home/azurice/Games/ffxiv/game/sqpack chara/weapon/w2001/obj/body/b0102/material/v0001/mt_w2001b0102_a.mtrl /tmp/opencode/w2001b0102_a.mtrl
target/debug/examples/extract_entry /home/azurice/Games/ffxiv/game/sqpack ffxiv/040000.win32.dat0 1569636480 /tmp/opencode/material-legacy16.mtrl
target/debug/examples/local_sqpack /home/azurice/Games/ffxiv/game/sqpack chara/human/c0101/obj/face/f0001/material/mt_c0101f0001_fac_a.mtrl /tmp/opencode/c0101f0001_fac_a.mtrl
target/debug/examples/local_sqpack /home/azurice/Games/ffxiv/game/sqpack bgcommon/hou/dyna/opt/sg/0009/material/opt_sg_m0009_3a.mtrl /tmp/opencode/bg_opt_sg_m0009_3a.mtrl
target/debug/examples/local_sqpack /home/azurice/Games/ffxiv/game/sqpack chara/equipment/e0001/material/v0001/mt_c0101e0001_top_a.mtrl /tmp/opencode/e0001_top_a.mtrl
```

按上表文件名逐份提取七份 `shader/sm5/shpk/<filename>` 到 `/tmp/opencode/<filename>`。
`bg.shpk` 的命令在根目录参数前加 `--max-bytes 134217728`。随后执行：

```sh
python tools/verify_materials.py /tmp/opencode/material-evidence-final /tmp/opencode/w2001b0102_a.mtrl /tmp/opencode/material-legacy16.mtrl /tmp/opencode/c0101f0001_fac_a.mtrl /tmp/opencode/bg_opt_sg_m0009_3a.mtrl /tmp/opencode/e0001_top_a.mtrl /tmp/opencode/character.shpk /tmp/opencode/characterlegacy.shpk /tmp/opencode/skin.shpk /tmp/opencode/bgcrestchange.shpk /tmp/opencode/water.shpk /tmp/opencode/bg.shpk /tmp/opencode/hair.shpk
python tools/find_material_variants.py /home/azurice/Games/ffxiv/game/sqpack /tmp/opencode/material-544-discovery.json
```

输出目录/报告必须不存在。验证器逐语义行比较 Rust/Python，完整 shader 字节及 node/alias/cluster 用 FNV-1a 对照；
文件身份和完整语义行另以 SHA-256 固定到 `tests/fixtures/material-observations.json`。
未经登记的样本明确标为 unregistered，不自动宣告固定回归通过。
工具为 Python 3.14.8、zlib 1.3.2；无服务器、GPU、游戏启动或进程附加。

## 受控回归与验证边界

`tests/mtrl.rs` 覆盖四种表长度、所有 half lane、HDR/负数、原始 dye 位、无 dataset、
异常 dataset、非法 offset/size/texture index、截断、字符串预算与 4096 次确定性变异。
`tests/shpk.rs` 覆盖五阶段和四种资源表次序、默认值存在/缺省、node 前置未知字段、pass/alias 引用、
资源名称范围、计数/分区/分配/字符串预算、截断和 4096 次确定性变异。
它们不替代真实 Legacy dye、非空 hull/domain 或 shader 求值证据。

## SHPK 目录库存、Geometry 与 0x0d01

`tools/inventory_shpk.py` 对 base shader index 的前 100000 条记录作目录 hash
过滤；该 index 实际 319 条，`shader/sm5/shpk` 目录有 64 条。只读 type-2
首解压块，声明资源上限 128 MiB、块表最多 16384 项，实际观察 59 份 0x0e01
与五份 0x0d01，全部为 DX11，没有观察失败。五阶段头计数合计
Vertex=1670、Pixel=32382、Geometry=2、Hull=0、Domain=0。这是目录头库存，
不把所有 64 份的完整 shader 字节或其他目录视为已验证。

Geometry 见证是 index ordinal 271、hash `e3466a824628d2e7`，dat0 offset
`231236736` 的 13951 字节 0x0e01 文件；完整身份与输出哈希固定在
`tests/fixtures/shpk-variant-observations.json`。它声明 1 vertex / 2 pixel /
2 geometry、3 nodes / 6 passes / 3 clusters；保留的 cluster 原始区长 1224。
Rust 提取与独立 zlib 全字节相同，全部 shader/资源/key/node/pass/alias/cluster
语义行一致，10 项资源名称 CRC 与五份 DXBC 长度均符合独立观察。

五份 DXBC 的 chunk 目录、非重叠范围、SHEX 字节长度与 SHA-256 也独立记录。
Vertex、Pixel、Geometry 对应 SHEX 首 token 的高 16 位分别观察到 1、0、2；
低 16 位均为 80。没有运行 shader、反汇编其指令或调用图形 API。这些字段
只作为阶段划分的附加静态证据，不声称已经解释完整 DXBC 格式。

该文件 pass 三项尾值为 `ffffffff,ffffffff,0/1`，末项与两份 Geometry 记录
的范围一致，但不据单一样本就改名或断言运行时映射。为核对公开命名，先重新
获取 VFXEditor 同 commit 根 LICENSE（MIT，Copyright 2021 Michael Kaminsky），
再完整阅读 `VFXEditor/Formats/ShpkFormat/Nodes/SphkPass.cs`；来源仍将三项称为
Unknown1/2/3。未复制、翻译或机械移植该文件代码，未增加第三方依赖；保持
`Pass::unknown` 原值并记录这一推断，不以 UI 的未知字段命名冒充已确认语义。

五份 0x0d01 位于 index ordinal 259/262/265/304/310。独立字节游标确认头长
84 字节，偏移 84 处已是首 shader 的 offset；没有 0x0e01 的 cluster count。
按 88 字节读取会将 shader length 等字节误作记录字段，无法抵达合法 DXBC。
按 84 字节读取后，全部分区、资源名称 CRC、八字节 vertex 前缀、三个 DXBC
长度、六个 node/pass、材质与 key 默认区、alias 末尾均一致到 shader blob。
五份文件均没有 cluster 段；hull/domain/geometry 的计数均为零。

`verify_shpk_variants.py` 对这五份与 Geometry 包共六份重新提取并独立逐行比较，
共 20 shaders、36 passes，完整结果报告摘要也固定在 fixture。连同原七份，
共十三份完整包、8609 shaders。五份 0x0d01 原始资源直接来自 DAT，版本布局
事实以这些文件的独立观察为依据。受控回归另把字段表重排为 84
字节头，检查有/无默认值、absence-vs-zero 的 cluster 声明、预算、全部截断和
4096 次变异；其非空额外阶段只是受控重排，不冒充 0x0d01 实样本。

```sh
nix develop -c cargo build --features local --example extract_entry --example validate_materials
python tools/inventory_shpk.py /path/to/game/sqpack /tmp/opencode/shpk-inventory.json
python tools/verify_shpk_variants.py /path/to/game/sqpack /tmp/opencode/shpk-variants-check
```

完整库存和六份复验报告的 SHA-256 均由工具强制检查，提取资源和 shader 字节
只写到安装与仓库之外的新目录。
