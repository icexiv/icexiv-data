# STM 来源与验证

## 来源和许可

第三方来源沿用 [材质来源审查](materials.md) 的 VFXEditor MIT commit
`ed6b39dcca04225c15c404a35a46945119462015`，先审查许可证后阅读
`Formats/MtrlFormat/Stm/{StmFileBase,StmFile,StmFileLegacy,StmDyeData}.cs` 与
`Entry/{StmEntryBase,StmEntry,StmEntryLegacy,StmLists}.cs`。
其中注释引用 Penumbra.GameData，未知上游许可限制保留。未复制、翻译其解析代码；
不采用其跨列读取和无效索引钳制行为，不宣称严格 clean-room。

## 样本和独立观察

Win32 安装版本 `2026.10.06.0000.0000`，只读提取
`chara/base_material/stainingtemplate.stm` 与 `stainingtemplate_gud.stm`。
哈希和语义行哈希固定在 `tests/fixtures/stm-observations.json`。
两文件各有 43 个模板；Legacy 有 178 Palette、37 Zero 列，Dawntrail 有
350 Palette、166 Zero 列；未引用的零尾部分别长 372、52754 字节。

`tools/verify_stm.py` 使用 Python struct 独立展开每列所有可表示 Stain ID，
对比 Rust `examples/validate_stm.rs` 的完整语义行；每列的 half/f32 位流
由 FNV-1a64 指纹表示。该入口的位流比较级别是指纹，不是原位数组逐字节比较。
同时核对 Stain EXH/EXD 的 129 行 × 9 列、258 个 UTF-8 字符串。
`e0001_top_a.mtrl` 的 32 行色表引用 Legacy 模板 200；武器材质的
1200/1202/1210/1212/1500 在 Dawntrail STM 中解析。

```sh
nix develop -c cargo build --example validate_stm
python tools/verify_stm.py /tmp/opencode/stm-check \
  /tmp/opencode/stainingtemplate.stm /tmp/opencode/stainingtemplate_gud.stm \
  /tmp/opencode/stain.exh /tmp/opencode/stain_0_chs.exd
```

`tools/stm_encoding_experiment.py new-directory legacy.stm dawntrail.stm` 将真实模板
重编码成 Dense/Uniform，逐 half 保留 ID 1–253 的值；Dense 的第 254 项特设为零。
再对两个产物运行同一独立验证器。它们是受控产物，不是游戏生成样本，不能据此
确认游戏使用这些编码的频率或第 254 个 Stain 的实际语义。

原始文件与解码产物只保存在仓库外；本仓库保存工具、观察哈希与自行编写的回归测试。

## 固定资源与完整位输出

外部证据目录 `../icexiv-data-evidence/stm-compatibility-19/` 固定同一安装
的两条资源路径。独立 Python 按块表/块头执行有界 raw DEFLATE，检查输入
耗尽、流结束与精确输出长度；两份完整逻辑文件与公共 SqPack 提取输出
逐字节相同。原压缩区、API 文件、详细列范围及全部输出保存在该目录。

| 资源文件名 | 逻辑字节 | 原始八字节头 | 模板 / 列 | 压缩块 | 未引用零尾部 |
| --- | ---: | --- | --- | ---: | ---: |
| `stainingtemplate.stm` | 81920 | `4d5301012b000000` | 43 / 215 | 6 | 372 |
| `stainingtemplate_gud.stm` | 204800 | `4d5301022b000309` | 43 / 516 | 13 | 52754 |

两份文件的模板体均起于字节 352。详细观察保存全部 86 个模板 ID、u32
偏移、u16 累计列末尾、731 个列范围、调色板及选择区。实际列为 Zero 与
Palette；零尾部按原字节保留，其存在不定义其他变体的尾部约束。

`resources/verify.py` 的 STM 解释复用归档的本项目 `tools/verify_stm.py`，
详细位展开沿用同一格式解释，不宣称重新独立发现 STM 算法。全部 819 行、
44137 字节语义输出与 Rust 验证器及固定 fixture 一致；这个结果中的 Rust
比较仍为每列完整位流指纹。

`examples/validate_stm_bits.rs` 直接输出公共 `half_bits` / `value` 接口的
完整位数组。顺序为文件、模板、列、Stain、lane，每项存储小端
`u32(half bits), u32(f32 bits)`，每个列×Stain 为三 lane、24 字节。
标量列的两个非值 lane 是 API 定义的零。`verify_bits.py` 将 Rust 输出
与 Python 新展开及原留存数组逐字节比较，共 185146 个列×Stain、
315768 对有效 half/f32 分量、4443504 字节；全流 SHA-256 为
`baa8f7e0fc3c54ee79a72cc1c1130bc53ffac589f7a5e3603a1e0dc1a6dd0cbb`。

`bits-review/verify.py` 另以整数 binary16→binary32 有限值展开核对每条
24 字节记录及完整连接流，并验证交换文件顺序的输出。实际有效分量为
195474 个 normal、120108 个正零和186 个 subnormal；负零与非有限 half
没有实样本覆盖。匿名合成输入只用于观察器预算检查：每文件接受上限
16 MiB，超限探测最多多读一字节；输出两文件合计上限16 MiB，非零退出
即使有完整记录前缀也不计为成功。

两个验证器默认只读复跑，不读取安装；归档 Rust 验证器、源文件及依赖有
固定身份。比较包含完整结果对象及规范 JSON 字节，并核对输入/依赖前后
hash/stat；首次捕获和指定复跑输出拒绝覆盖，禁止 `-O/-OO`。
位输出子入口的 inventory 检查只针对普通文件；独立审核及整体清单额外
核对文件/目录的前后完整集合，并拒绝符号链接与特殊条目。固定输入、
结果及整体清单身份见 `tests/fixtures/stm-compatibility-observations.json`；
整体重算入口为外部目录的 `python3 -B verify_bundle.py --replay`。

这些结果确认指定版本和两份固定输入的解析兼容，不能证明其他历史头部、
索引表示、Uniform/Dense 生产使用、Palette Stain 254 或 shader 求值。

## 客户端请求与查询的静态关联

目标为同一 Win64 executable，51910400 字节，SHA-256
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`，
首选 base `0x140000000`。外部证据 `stm-compatibility-19/review/` 独立
解析 PE，将固定 raw 全部对回 EXE，并重新反汇编后逐字节比较保存的 asm。
使用 Python/GNU objdump，没有运行、模拟或附加客户端；普通离线证据
不称为 REA provider Evidence。

| 观察点（首选 VA） | 静态事实 | 限定 |
| --- | --- | --- |
| `140426e06` 的路径请求窗口 | `_gud.stm` 结果存入 RBX 对象 `+0x300`，普通 `.stm` 存入 `+0x308`；同一 u32 key | RBX 与 consumer 全局对象的运行实例关系未闭合 |
| `1402fe9b0 → 140316090 → 14031c5d0` | 所选请求的直接调用链，lookup 可经 factory 虚表 `+8` 尾调用 | 先行对象查找可绕过 factory；失败另有 fallback |
| `14031b9a6` 附近 | 同一调用点将 key 与首指针为 `14214eb68` 的对象传给注册 helper | helper 本体没有捕获，树插入及实际选择未知 |
| `14214eb68 + 8 → 1403142c0` | factory 安装 resource 虚表 `14214ea00` | 初始化 helper 及运行选择另有边界 |
| `14214ea00 + 0xb8 → 140300be0` | getter 取对象字段 `+0xb0`，非空时尾调内层虚表 `+8` | 内层目标、写入者及返回缓冲区与原文件身份未知 |
| `141883cf0` | 接收指针 `B+4` 为 u16 数量，`B+8` 为 u32 key 表；u32 offset 对应 `B + 8 + 8N + 2×offset` | 查询参数取 DX 不改变存储 key 的四字节宽度 |

列 helper 从记录读取 u16 累计末尾。指定 consumer 的对象指针比较选择
五项/十二项描述及 `record+10` / `record+24` 两种载荷基址；这条选择
不是从 STM 头部 version 读取。小查询函数没有读取 magic/version，不代表
完整加载过程忽略它们；返回缓冲区是否经过预处理也需要独立身份边。

范围为三个旧窗口的 1000 字节、六入口的 4978 唯一代码字节和两数据区的
216 字节。getter 的首 16 字节窗截断末指令，同入口 32 字节窗补足后仍计
六入口。一次精确 key 查询产生三个字节候选，各自的 mov 边界已确认；
独立审核只复核保存命中，不重复搜索，也没有使用第二查询预算。

`review/result.json` 为 449821 字节，SHA-256
`8cfa2416e6ee96180989a8ef6236117669c4c363cbb6245057b638f8e2cbee86`。
默认复跑比较完整对象及规范字节，记录原字符串、全部选中指令、表项与
输入前后 hash/stat。这些局部字段/调用证据不能证明额外历史格式兼容。

## 历史候选的非实现来源

[隔离功能审计](legacy-coverage.md)记录的额外 STM 表示只有实现与受控正例
定义，没有对应的生产输入身份。`stm-compatibility-19/PUBLIC-SOURCES.md`
保存两次预声明搜索及一个页面元数据检查的范围：XIV Modding 的
“Dye templates and Tile References”署名 gwyneth_allora，注明改编自 sel
的参考文档；元数据检查未找到明确许可声明，没有把该页登记为批准的
二进制规格。Physis API 及模板仓库实现未打开。

这项有界检索没有取得额外历史表示的非实现规格；两份已知资源也不构成
该候选的见证。它不排除其他资料或历史客户端存在不同表示。
