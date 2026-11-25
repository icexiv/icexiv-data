#import "../_components/pele/lib.not" as pele;

# TMB 动作时间线

Win32 `.tmb` 保存动作中的 actor、轨道和命令。文件以 `TMLB` 开头，与
[PAP 内嵌时间线](../pap/README.md)共用条目头与尾区寻址方式。TMAL 列出
actor ID，TMAC 列出 track ID，TMTR 列出 command ID；条目位置与 ID 是两种坐标。
未被 actor 引用的轨道也可以出现在条目表中。

下图为 72 字节 C012，位置单位为 bit，原点为条目头起点，数值小端。
每个偏移的值以字节计，原点是 **ID 字段起点**（条目 `+8`）。

#pele.diagram(r#"""
packet
0-31: "Tag C012"
32-63: "Entry size"
64-79: "ID"
80-95: "Start time: i16"
96-127: "Duration: i32"
128-159: "Unknown"
160-191: "AVFX name offset"
192-223: "First bind target"
224-255: "Second bind target"
256-319: "Scale: offset and count"
320-383: "Rotation: offset and count"
384-447: "Position: offset and count"
448-511: "RGBA: offset and count"
512-543: "Visibility flags"
544-575: "Unknown"
"""#)

绑定目标各为 4 字节：`origin: i8, kind: i8, id: i16`。origin 的 1/2
表示施术者/目标，kind 的 0/1/2 表示角色/武器/副手。ID 可为 −1。
名称指向尾区中 NUL 终止的字节串；四个数组描述各为两个 u32，依次是尾区引用
和分量数。前三个数组为三个小端 f32（旋转单位为弧度），颜色为四个 f32。

| 命令 | 总长 | 名称偏移字段在负载中的位置 | 含义 |
| --- | ---: | ---: | --- |
| C002 | 28 | +16 | 子时间线逻辑路径，可省略 `.tmb` |
| C009 | 24 | +12 | PAP 动画记录名 |
| C010 | 40 | +24 | 动画记录名 |
| C012 | 72 | +12 | AVFX 路径 |
| C063 | 32 | +12 | SCD 路径；负载 +4 为 loop/duration |

表中长度与位置对应所列 Win32 布局。命令头给出的时间是时间线坐标，不是
毫秒；条件轨道被引用不等于它在每次播放中都会执行。

TMAC 和 TMTR 的负载均从 `ID: u16, start: i16` 开始；TMAC `+4` 为
`ability delay: i32`。TMTR 的 `+12` 是相对本条目负载的条件块偏移，0 表示无块。
所见条件块以 `8: u32, count: u32` 为头，随后各项为 12 字节：
操作码 u32、整数参数 u32、浮点参数 f32。条件表记录输入表达式；
表达式依赖的角色状态与变量值不存储在该表里。
