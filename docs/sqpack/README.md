# SqPack

SqPack 将资源路径映射到索引条目，再从 DAT 文件中提取资源数据。

## 目录结构

```text
game/sqpack/
├── ffxiv/                       # 基础仓库，ID 0
│   ├── 0a0000.win32.index        # 目录哈希 + 文件名哈希索引
│   ├── 0a0000.win32.index2       # 完整路径哈希索引
│   ├── 0a0000.win32.dat0         # 资源数据
│   ├── 0a0000.win32.dat1
│   └── ...
├── ex1/                         # 资料片仓库，ID 1
├── ex2/                         # 资料片仓库，ID 2
└── ...
```

同一包的索引和数据文件共享文件名前缀。两种索引不一定同时存在，DAT 文件数量也不固定。

| 文件 | 内容 |
| --- | --- |
| `.index` | 目录哈希、文件名哈希 → DAT 编号及偏移 |
| `.index2` | 完整路径哈希 → DAT 编号及偏移 |
| `.datN` | 资源条目、块表和资源数据 |

## 文件命名

```text
{category:02x}{repository:02x}{chunk:02x}.win32.{index|index2|datN}

0a 00 00 .win32.index
│  │  └─ 分片编号（chunk）
│  └──── 仓库 ID：00 → ffxiv，01 → ex1，02 → ex2
└─────── 类别 ID：0a → exd，02 → bg
```

三段编号均为两位十六进制数。`win32` 是资源平台标记，不是当前游戏进程的位数。

- **repository**：`ffxiv` 或 `exN`，不是补丁号，也不严格对应资源加入游戏的时间。
- **chunk**：同一类别、仓库下的包分片；改变 chunk 就是另一个索引包。
- **datN**：同一包内的数据文件编号，由索引条目指定，普通位置字段可表示 `0`～`7`。

例如 `ex2/020200.win32.dat1` 表示 `bg` 类别、仓库 `ex2`、chunk `00` 的第 `1` 号数据文件。

## 类别

资源路径首段对应包名的 category 字节，ID 为十六进制：

| ID | 路径首段 | 内容 |
| --- | --- | --- |
| `00` | `common` | 通用资源、字体 |
| `01` | `bgcommon` | 场景共享资源 |
| `02` | `bg` | 场景布局、模型、纹理、碰撞 |
| `03` | `cut` | 过场 |
| `04` | `chara` | 角色、怪物、装备 |
| `05` | `shader` | 着色器 |
| `06` | `ui` | 界面布局、图像 |
| `07` | `sound` | 音效 |
| `08` | `vfx` | 特效 |
| `09` | `ui_script` | 界面脚本，历史类别 |
| `0a` | `exd` | Excel 表格 |
| `0b` | `game_script` | 游戏脚本 |
| `0c` | `music` | 背景音乐 |
| `12` | `sqpack_test` | 测试 |
| `13` | `debug` | 调试 |

类别不是文件类型：模型、纹理可以分别存在于 `bg`、`bgcommon`、`chara` 等包中。历史、测试和调试类别不一定有发行版数据。

## 资源路径

资源路径是包内名称，不是安装目录中的物理路径。使用 `/` 分隔，查找时转为 ASCII 小写：

```text
{category}/{path}
{category}/exN/{path}

exd/item.exh                          → 类别 0a，仓库 ffxiv
music/ex2/bgm_ex2_system_title.scd     → 类别 0c，仓库 ex2
```

首段决定类别；第二段为 `exN` 时选择该仓库，否则选择 `ffxiv`。`exN` 仍参与路径哈希，不能删除或移到类别前面。类别内部的目录和命名规则由各资源系统定义。

### 从路径定位数据

以 `exd/item.exh` 为例：

1. 选择类别 `0a`、仓库 `ffxiv` 下的包；路径不能直接确定 chunk，当前库依次探测 `00`～`ff`。
2. `.index` 查找目录 `exd` 和文件名 `item.exh` 的 JAMCRC；`.index2` 查找完整路径的 JAMCRC。
3. 索引给出同包的 DAT 编号和条目偏移，解包后得到 EXH 字节。

对应关系为：

```text
exd/item.exh
  → ffxiv/0a00{chunk}.win32.index 或 .index2
  → 同前缀的 .datN + 条目偏移
  → EXH 字节
```

普通索引条目只指定 DAT 编号和偏移，不能跳到另一 category、repository 或 chunk 的包。索引和 DAT 的二进制布局见 [index、index2 与 DAT](container-formats.md)。

### 路径从哪里来

主索引保存的是哈希，不能唯一还原名称；目录哈希也不保存逐级目录树。辅助碰撞/名称记录不是完整文件名清单。因此遍历索引得到的是哈希列表，不是可按扩展名筛选的资源目录。

候选路径通常来自已知入口、资源内引用、命名规则或外部路径字典。例如表格从 `exd/root.exl` 获取表名，再读取 EXH 和 EXD，见 [Excel 表格资源](../excel/README.md)。

其他资源通常沿场景布局、模型/材质、特效或表格中的引用继续查找；引用不一定是文本路径，也不一定有全局清单。
