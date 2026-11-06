#import "../_components/pele/lib.notc" as pele;

# CMP 角色外观参数

`chara/xls/charamake/human.cmp` 保存角色外观调色板，以及按种族、body type、
clan 索引的体型参数范围。Win32 八种族布局使用小端 IEEE 754 binary32。
文件没有签名、数量表或偏移头；需结合版本识别颜色区和参数区。

## 数据组织

此 Win32 变体有 45568 个 RGBA8 颜色（178 组，每组 256 个），之后是 80 条
56 字节体型参数记录，总长 `0x2d980` 字节。参数区起点为 `0x2c800`。
颜色区长度存在版本差异，不能把此偏移推广到所有历史或未来 CMP。

#pele.diagram(r#"""
flowchart TB
    colors[RGBA8 colors: 178 x 256 x 4 bytes] --> ranges[Body ranges: 8 x 5 x 2 x 56 bytes]
"""#)

颜色以 R、G、B、A 字节顺序存放。alpha 并非全部为 255，透明度相关的颜色范围
不能按“不透明颜色”统一截断。调色板中包含共用颜色和与 tribe/sex 有关的颜色；
功能到调色板的选择规则与颜色字节编码是两层信息。

## 体型记录索引

每种族有五个 body type，每种 body type 有两个 clan。以 race index 0–7、
body type 1–5、clan index 0–1 表示时，记录编号为：

```text
record = race_index × 10 + (body_type − 1) × 2 + clan_index
offset = 0x2c800 + record × 56
```

同一记录内同时保存男性和女性范围。Win64 `2026.10.06.0000.0000` 的消费点将
1–16 的 tribe 编号减一后除以二得到 race index，以最低位选择 clan。
游戏对非法 customize 值的回退不属于文件编码；
记录的固定坐标与运行时 customize 字节应分别表达。

## 记录布局

图中位置以 bit 为单位，原点为单条记录起点；每项是小端 f32。

#pele.diagram(r#"""
packet
0-31: "Male height minimum"
32-63: "Male height maximum"
64-95: "Male tail minimum"
96-127: "Male tail maximum"
128-159: "Female height minimum"
160-191: "Female height maximum"
192-223: "Female tail minimum"
224-255: "Female tail maximum"
256-287: "Bust minimum X"
288-319: "Bust minimum Y"
320-351: "Bust minimum Z"
352-383: "Bust maximum X"
384-415: "Bust maximum Y"
416-447: "Bust maximum Z"
"""#)

高度、尾长是标量范围；胸围是三轴范围。范围值是缩放参数，不是角色世界空间的
米制身高或骨骼位置。体型记录保存端点，不存每个 slider 位置对应的完整矩阵。
消费方据此求值，再与骨架、动画和展示缩放关联，不应把这些范围写回 bind pose。

## 百分比消费关系

指定 Win64 版本的身高、尾长与胸围消费点都从 56 字节记录读取上述端点。
对于合法百分比 `p ∈ [0,100]`，标量或各个 XYZ 分量的代数关系为：

```text
value = minimum + (maximum - minimum) × (p / 100)
```

男性身高端点在记录 `+0/+4`，女性在 `+16/+20`；尾长分别在 `+8/+12`、
`+24/+28`。胸围先取 `+32/+36/+40` 的三个最小值，再分别与 `+44/+48/+52`
的最大值插值。浮点指令执行次序会影响最终舍入位；该代数式不指定求值精度。

消费点还存在输入策略：身高值 `255` 返回单位标量，`101–254` 按百分比 0
处理；尾长与胸围分支中大于 100 的值按 0 处理。胸围的男性分支返回单位向量，
女性分支读取三轴范围。它们是程序对 customize 输入的策略，不是 CMP 文件中
保留了第 255 项记录或一份单位矩阵。body type 与种族回退、分支启用和姿态
应用仍须区分于记录本身的固定坐标。
