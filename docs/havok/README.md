#import "../_components/pele/lib.notc" as pele;

# Havok binary tagfile v3

FFXIV 的 SKLB 等资源内嵌 Havok binary tagfile。v3 是带类描述的对象流：类描述
定义成员与继承关系，随后对象按这些声明编码值。标量实数是小端 binary32；
整数、字符串长度、类 ID、对象 ID 和标签使用下面的变长整数编码。

## 头与记录流

前八字节为固定签名 `1e 0d b0 ca ce fa 11 d0`。后接 file-info 标签 1 和版本 3，
均为变长整数，典型前缀因此是 `… 02 06`。余下为记录流，直到 end 标签 7。

| 解码后的 tag | 记录 |
| --- | --- |
| 1 | File info |
| 2 | Class declaration |
| 3 | Inline object |
| 4 | Remembered object |
| 5 | Object back reference，后接对象 ID |
| 6 | Null |
| 7 | End |

#pele.diagram(r#"""
flowchart LR
    magic[8-byte magic] --> info[File info: tag 1 / version 3]
    info --> declarations[Class declarations]
    declarations --> objects[Objects and references]
    objects --> end[Tag 7]
"""#)

类与对象 ID 为从 1 开始的表索引，0 表示 null。tag 4 才把对象加入 remembered
表；对象内的指针可以引用流中较晚出现的 remembered 对象。null 与空数组、空
字符串、缺省成员不是同一个状态。

## 变长有符号整数

首字节 bit 0 为符号，bit 1–6 为绝对值低六位，bit 7 为 continuation。
后续每字节 bit 0–6 提供更高的七位，bit 7 表示继续。负数使用负绝对值，
不是普通二补码或 protobuf zigzag。

以下图为首字节的**数值位域**，位号从最低有效位 0 开始，与 f32 的小端字节序
分别定义：

#pele.diagram(r#"""
---
config:
  packet:
    bitsPerRow: 8
    bitOrder: ascending
---
packet
0: "Sign"
1-6: "Magnitude bits 0..5"
7: "More"
"""#)

例如 `02` 表示 1，`03` 表示 -1，`80 01` 表示 64。

## 字符串

先读变长整数 n。n 为正时，随后 n 个字节组成新字符串，并加入字符串表；
n 非正时引用表的 `-n` 项。表的初始两项为 `0 = 空字符串`、`1 = null`。
字符串没有额外的 NUL 终止符，文件中的编码字节可以与文本解码分开保存。

## 类声明与成员类型

tag 2 后依次为类名字符串、类版本整数、父类 ID、成员数量和成员描述。
每成员有名字字符串与 type 整数；type 带 `0x20` 时再有 tuple 长度，低四位
为 pointer/struct 时再有目标类名字符串。目标类名可引用较晚声明的类。

| Type 低四位 | 值 |
| --- | --- |
| 0 | Void，不存普通值数据 |
| 1 | 原始 byte |
| 2 | 变长有符号 integer |
| 3 | f32 |
| 4 / 5 / 6 / 7 | 4 / 8 / 12 / 16 个 f32 |
| 8 | remembered 对象 ID |
| 9 | 内联 struct |
| 10 | 字符串 |

`type & 0x10` 表示数组，长度先编码；`type & 0x20` 表示类声明给定长度的 tuple。
类的成员顺序先继承父类成员，再接本类成员。

## 对象与 presence 位

对象记录先存类 ID。内联 struct 的类由成员声明给出，不再存类 ID。
随后是 `ceil(成员数 / 8)` 字节的 presence bitmap，每字节低位先对应前面的成员。
只有置位成员才有后续编码值；缺省成员的含义由类定义决定，不能为所有类型统一
补一个单位矩阵或非空对象。

## 数组与列式结构数据

- byte 数组直接存原始字节，real 数组直接存 f32。
- v3 integer 数组先存整数宽度描述（4 或 8），再存各变长整数。
- pointer 数组存各对象 ID；字符串数组存各字符串编码。
- vec4 数组先有存储分量数（3 或 4），再有每元素的相应 f32；8/12/16 分量数组
  直接存系数。缺失的第四通道与显式存零应区分。
- struct 数组使用一份成员 presence bitmap，随后**逐成员列**存所有元素的值。
  嵌套 array/tuple 成员则逐元素读取该成员自己的长度/值。

#pele.diagram(r#"""
flowchart TB
    mask[Shared struct-member presence bits] --> names[All elements: name column]
    names --> locks[All elements: lockTranslation column]
    locks --> more[Other present member columns]
"""#)

因此 hkaBone 数组不是重复的 `(name, lockTranslation)` 行式记录；按行读取会让
第二个骨名和后续 referencePose 全部错位。
