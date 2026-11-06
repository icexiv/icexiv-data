# SqPack index2 碰撞记录来源与验证

## 输入与来源

来源仅为用户授权只读 Win32 安装 `2026.10.06.0000.0000` 的
`game/sqpack/*/*.win32.index` 与对应 `.index2`。没有阅读第三方解析实现、
客户端代码或 legacy，没有加入依赖，不对专有安装数据推定再分发许可。
原始索引、路径列表和完整逐表报告留在外部目录。

## 独立字节观察

`observe_index2_collisions.py` 读取所有发现的 59 对主表，并检查两类文件
集合一一对应。table 总读取预算为 128 MiB、单表 64 MiB、最多 128 对文件，
名称总预算为 16 MiB。公共头、索引头、表对齐和每个声明跨度均检查。

主 index 共有 1802972 条记录，无 flagged 记录或重复 64 位 hash。
index2 的普通记录共有 1802910 条，另有 31 条 flagged hash。四个含碰撞
的包为 `ffxiv/010000`、`020000`、`040000`、`080000`，flagged 数分别为
1、12、14、4；每个 flagged packed 字均为 1。

直接观察 index2 专用头 `+0x54/+0x58` 的偏移/长度与文件边界，辅助区按
256 字节划分：有效记录的 `+0` u32 为完整路径 hash、`+4` 为未知字、
`+8` 为 packed DAT 位置、`+0x0c` 为序号字，`+0x10` 起为 NUL 结尾路径。
终止记录的未知字存在 0 和 `0xffffffff` 两种值，不能只接受一种固定的
16 字节 sentinel；路径 NUL 后也可含非零字节。

所有 62 个有效辅助记录均完成两种独立核对：

1. 用 Python `zlib.crc32(path) ^ 0xffffffff` 计算全路径 JAMCRC，与其
   index2 hash 及 flagged 主记录关联。每个组有两个不同路径。
2. 在最后一个 `/` 分割目录和文件名，分别计算 JAMCRC，组合为主 index
   的 64 位 key；对应主记录的 packed DAT 位置与辅助记录完全相同。

普通 index2 位置与 62 个辅助位置合并后，逐包的 packed **多重集**与主
index 完全一致，不仅比较集合。这说明指定安装已发现的 index2 主表和
碰撞表没有额外主索引外位置；不检查 orphan DAT 记录、未安装包或其他版本。
因此这些碰撞记录没有为已索引 volume 引入新的完整 counterpart。

`tests/fixtures/index2-collision-observations.json` 固定报告 SHA-256
`8eeba80d03b5d0bb8a09dbfe8cbcdddff6c603e4964e9e1301dfb403b159de22`。
报告保存所有主表/辅助表摘要、完整路径、未知字、序号、hash 和 DAT 位置。
这属于库存/格式观察，不是 DAT 提取或库碰撞消歧成功的声明。

## index2-only 查询验证

`verify_index2_lookup.py` 使用固定库存报告生成输入清单；期望 DAT 位置来自
上述独立全路径/分段 hash 及主表核对。`validate_index2` 比较全部 59 个
容器的借用快照与异步加载：主表记录和辅助表原始字节完全一致，包含
终止记录、未知字和 NUL 后数据。62 条碰撞路径的两种查询均与独立位置相等。

资源查询使用包装后的 Local 后端：任何 `.index` 请求直接返回确认不存在，
不打开主 index；`.index2` 与 DAT 保持真实只读访问。全部 62 条路径成功
提取，并与独立位置直接调用 `read_entry` 的字节完全相同。该字节比较
复用库 DAT 解码器，只验收名称查询/路由，不冒称新的独立解码 oracle。
每条输出的长度、SHA-256 和位置保存在外部报告中，固定身份见
`tests/fixtures/index2-lookup-observations.json`：
`dfff5fae27be97b7a5822307a1e40c7749e0deb556fbd57ab42eb8d4edfd3f22`。

`tests/sqpack_resource.rs` 的受控容器回归覆盖两个真实碰撞路径的查询，
两种 sentinel 未知字、非零尾部、重复路径歧义、同 hash 无匹配、缺少
辅助表、预算、逐字节截断、跨度越界/重叠、无 NUL、错误 hash/flagged
位置及辅助区读取错误。受控容器不是游戏原始文件。

主表查询另有与碰撞消歧独立的路径：构造视图时线性检查 key 是否有序，
有序表使用 lower-bound 查询，未排序表保留线性查询，不改写表顺序。
`src/sqpack/index.rs` 的回归将有序/未排序、空表、首尾和高位 key、重复
hash 及 flagged 记录的结果与线性路径对照，保留首匹配错误的优先顺序。
这由本项目按普通二分查找编写，没有阅读或移植客户端的索引查找实现。

```sh
python tools/observe_index2_collisions.py /path/to/game/sqpack \
  /tmp/opencode/index2-collision-inventory
python tools/verify_index2_lookup.py /path/to/game/sqpack \
  /tmp/opencode/index2-collision-inventory/report.json /tmp/opencode/index2-lookup
```
