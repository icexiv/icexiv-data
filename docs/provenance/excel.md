# EXH / EXD 来源与验证

## 来源和边界

需求为 xiv-companion 的目录、模型号与家具 ModelKey 等底层字段读取。
本轮以仓库既有 `src/excel` 为受检实现，使用独立 Python struct 观察授权的
Win32 安装版本 `2026.10.06.0000.0000`，没有调用第三方 FFXIV/Excel 解析器、
datamining CSV 或名称 schema，没有阅读 legacy。格式推断与数值对照不被称作
严格 clean-room；这些观察也不验证全部跨表业务名称或角色装配策略。

样本从 `exd/` 的真实资源路径定位，经 Rust SqPack 与独立 Python index 查找、
raw-deflate 重建，逐字节对照提取结果。七份 EXH、九份 EXD 的路径与文件哈希、
全部元数据/字段结果的行哈希固定在 `tests/fixtures/excel-observations.json`。
原始资源、所有输出行与完整报告仅写到安装和仓库之外的新目录。

## 真实页面和语义输出

| Sheet | 页面 | 变体 | 验证要点 |
| --- | --- | --- | --- |
| Item | 0、2500、50000，chs | 1 | 三个目录区段，所有 91 列，原始字符串、u64 和八种 packed bool |
| ModelChara | 0，无语言 | 1 | 5284 行的全部 23 列，模型数据标量与 f32 |
| HousingFurniture | 196608，无语言 | 1 | 1793 行全部 15 列；ID 区间长度 1795，不等于实际行数 |
| EquipSlotCategory | 0，无语言 | 1 | 25 行的全部 14 个 i8 字段 |
| GilShopItem | 262144，无语言 | 2 | 16157 subrow，单 parent 最多 59 条 |
| GatheringItemPoint | 1，无语言 | 2 | 1716 subrow，单 parent 最多 38 条 |
| CustomTalkDefineClient | 0，无语言 | 2 | 193 subrow、共享字符串池、稀疏 parent ID |

九页共 10892 个 parent、26668 条输出行、448949 个字段、6193 个字符串字段
（3360 个非空、80966 字节）和 15852 个 f32 值。559 个 u64 字段的高 32 位
非零，逐值比较确认没有截断。对所有选定页面逐列、逐数值／浮点位、逐字符串
字节比较 Rust 与 Python；未把只比较行数或摘要当成完整字段对照。

Item 只抽取上述三页，不声称全表验收；其他六表读取其全部声明页面，并核对
输出总行数等于 EXH 的全表声明数量。generic 列类型编码支持的普通 bool/i64
另有受控回归，不冒充本轮实际出现的列类型。

## Subrow 字符串池观察

为寻找带字符串的真实 variant-2 表，`tools/find_excel_subrow.py` 读取 root.exl
中的名字，对最多 5000 个存在的 EXH 作有界检查；找到 CustomTalkDefineClient。
其 parent 721028 的绝对记录偏移为 226，payload 起点 232，固定大小 8，subrow
数量 4，字符串池起点为 `232 + 4*(2+8) = 272`。四条存储的字符串偏移为
`0,32,51,82`，从同一池分别指向四个完整的 CUTSCENE/BGM 名字。

以每条 subrow 的固定区末尾为原点的假设会让第一条指向其他 subrow 记录，
后续偏移也落在字符串中部；该假设不能由按它构造的合成 fixture 验证。
真实观察支持所有 subrow 共用 parent 末尾池。回归据此检验偏移零、不同
subrow 共享同一字符串及借用地址，并保持不能越出 parent 的边界校验。
不连续及极值 subrow ID 的控制实验不被用来声明本轮游戏页面出现了相应键型。

## 长度、预算与受控实验

九个实际页面均满足 EXD `+0x0c` 的大端长度覆盖完整数据区：
`32 + index_size + data_size = extracted_file_size`，包含 parent 记录及填充。
EXH 语言表的实际编码包括 `00 00` 与 `05 00`；低位 ID 在前，后一字节为零，
本轮不由这些样本推定高字节的其他扩展含义。

受控回归覆盖原生/WASM 的 i64/u64 极值、signed 标量、非有限 f32 位、所有
packed bool、名字/path 校验、乱序 parent 与稀疏 subrow 键、重复键、固定区／
父行／数据区越界、截断与显式预算。元数据、列／行数量、排序与 subrow 验证
工作有独立上限，惰性字符串解码受页级长度上限约束。

## 复现

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_excel
python tools/verify_excel.py /path/to/game/sqpack /tmp/opencode/excel-check
```

Python 观察器只用标准库。验证不启动游戏、服务器、GPU 或外部目标程序。
