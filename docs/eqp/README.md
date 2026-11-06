#import "../_components/pele/lib.notc" as pele;

# EQP 装备显示参数

`chara/xls/equipmentparameter/equipmentparameter.eqp` 按装备 set ID 保存五个槽位
的显示规则声明，涉及身体、其他装备槽、头发、耳、尾等。以下为 Win32 小端布局。
文件的 64 位字包含多个槽位自己的声明，并非一个已经合并求值的角色显示状态。

## 压缩块组织

逻辑空间有 64 块，每块 160 个 u64 条目，合计 10240 个位置。
首个 u64 是 64 位块存在 bitmap，复用逻辑位置 0；位置 0 不能再读作普通装备记录。
bit 0 对应逻辑块 0，bit 63 对应逻辑块 63；位号按数值从最低有效位开始，
与字节的小端存储顺序分别定义。

置位的块按逻辑块编号递增连续存放，没有额外的每块偏移表。
文件大小为 `popcount(bitmap) × 160 × 8`，bitmap 已包含在第一块内。
逻辑块缺失表示文件没有该块；运行时取哪个缺省 set 是独立规则，不能自动补成零。

#pele.diagram(r#"""
flowchart TB
    first[Physical block 0: bitmap + 159 entries] --> block[Other present blocks: 160 entries each]
    mask[Bitmap bit k] --> rank[Physical block = popcount of bits below k]
    rank --> block
"""#)

对 `set_id > 0`，先计算 `block = floor(set_id/160)` 与 `within = set_id mod 160`。
存在时，物理条目为 `popcount(bitmap bits below block) × 160 + within`，
再乘以 8 得到文件字节偏移。Win32 样本有 18 个存储块，文件长 23040 字节。

## 槽位区域

以下单位为 bit，原点为单个 u64 条目的最低有效位；多字节值按小端读取：

#pele.diagram(r#"""
packet
0-15: "Body declarations"
16-23: "Leg declarations"
24-31: "Hand declarations"
32-39: "Foot declarations"
40-63: "Head / hair / ears declarations"
"""#)

下表位号相对于整个 u64。命名来自公开字段资料；这些是声明位，不等于脱离其他
启用位或装备组合即可确定的最终显隐。未列出的位保留原始数值。

| 位 | 公开含义 |
| --- | --- |
| 0 / 16 / 24 / 32 / 40 | 启用 body / leg / hand / foot / head 声明 |
| 1 | Body hide waist |
| 3 / 5 / 6 / 7 | Body hide short gloves / mid gloves / long gloves / gorget |
| 8 / 9 / 10 | Body 提供 legs / hands / head 参数区域；公开名 show legs / hands / head |
| 11 / 12 / 13 / 14 | Body show necklace / bracelet / tail；disable breast physics |
| 17 / 18 / 19 | Leg hide kneepads / short boot / half boot |
| 21 / 22 | Leg 提供 foot 参数区域 / show tail |
| 25–26 | Hand coverage selector；公开单个位名 hide elbow / forearm |
| 28 / 29 / 30 | Hand show bracelet / left ring / right ring |
| 33–34 / 35 | Foot coverage selector / 比较位；公开单个位名 hide knee / calf / ankle |
| 41–43 | Head hair coverage code；公开单个位名 hide scalp / hide hair / show hair override |
| 44 / 45 / 47 | Head hide neck；show necklace / earrings |
| 48 / 49 | Head show human / Au Ra earrings |
| 50 / 51 / 52 / 53 | Head show human / Miqo'te / Au Ra / Viera ears |
| 56 / 57 | Head show Hrothgar / Viera hat |
| 59 | 类型 8 的 zear 消费分支选择角色状态替代耳位检查；业务名称未定 |

## 参数区域来源与耦合字段

Win32 安装版本 `2026.10.06.0000.0000` 的静态消费关系中，body 的位 8/9/10
选择该记录内的 legs/hands/head 区域；未选中或来源记录缺失时，分别查询相应
槽位的 set ID。所选 legs 参数的位 5（整字位 21）再选择该来源的 foot 区域，
否则使用 foot 槽自己的记录。一个槽位可以借用另一份记录中的参数区域。

#pele.diagram(r#"""
flowchart TB
    body[Body set entry] --> head[Head region: body bit 10 selects source]
    body --> hands[Hand region: body bit 9 selects source]
    body --> legs[Leg region: body bit 8 selects source]
    legs --> feet[Foot region: selected leg bit 5 selects source]
"""#)

以下数值位域在读取后整数上取值，位号相对整个 u64，低位在前；其数值位顺序与
文件小端字节序分开定义：

| 数值位 | 字段 | 取值 |
| --- | --- | --- |
| 3–4 | Body glove 比较字段 | 0–3 |
| 18–19 | Legs boot 比较字段 | 0–3 |
| 25–26 | Hand coverage selector | 0–3 |
| 27 | Hand 比较位 | 0/1 |
| 33–34 | Foot coverage selector | 0–3 |
| 35 | Foot 比较位 | 0/1 |
| 41–43 | Head hair coverage code | 0–7 |

selector 的两个位共同选择分支。处理还受相应 enabled 位、上述比较字段、
头部/种族参数及角色状态影响；单独的 selector 不构成完整显隐策略。

Head 的三个 hair 位也被作为一个整数共同消费。下图为数值位域，原点为
24 位 head 参数区域的最低有效位，单位为 bit；低位编号为 0，与整字的
40–63 位对应。图中的位号不改变文件小端字节序：

#pele.diagram(r#"""
---
config:
  packet:
    showBits: true
    bitsPerRow: 24
    bitOrder: ascending
---
packet
0: "Enabled"
1-3: "Hair coverage code"
4-15: "Neck / accessory / ear declarations"
16-17: "Race hat declarations"
18: "Other"
19: "Zear state selector"
20-23: "Unknown / other"
"""#)

`hair_code = (head >> 1) & 7`。指定 Win64 消费点按 code 1–6 查六项跳转表，
code 0/7 走该表的缺省分支；code 2 与 4 共用入口，code 3 则有独立入口。
这些值不是按单个位名顺序应用的三个 Boolean 操作。头发消费还受 head
enabled、帽子声明、角色分类与其他状态约束。实际 EQP 表覆盖 code 0–6；
code 7 在该表没有命中，不由此认定其为非法编码。

同一头发消费点按分类条件检查 head bits 10/11/12（整字 bits 50/51/52），
分别关联 `atr_mim`、`atr_top`、`atr_hrn`；另有分支组合 hair code 与帽子
声明。这些局部关系不能单独构成完整耳部或头发显隐规则。

## 耳与尾的条件声明

Body 的 tail 位 13 和 Leg 的 tail 位 22 是两个独立字段。在指定 Win64
普通 tail 槽的消费点，Body enabled 为 1 且位 13 为 0、或 Leg enabled
为 1 且位 22 为 0，均可清除同一模型启用位。enabled 为 0 时该区域不执行
此限制；置位的 tail 声明也不能取消另一区域已经施加的限制。这里的参数
区域来源仍由上一节的跨槽选择决定。

客户端模型槽编号 12 还用于类型值 8 的 `zear` 耳模型；其他对应路径使用
`tail` 模型。该耳模型走另一条条件分支，检查 Head enabled、位 57、角色
状态和位 53；位 59 切换到另一角色状态检查。因而不能把普通 tail 槽的
两个参数位用作所有角色耳部的统一求值规则。位 59 的观察只确定这项分支
作用，不为它推定业务名称。

头发与脸模型的耳部 attribute 消费均可引用 `atr_mim`、`atr_top`、
`atr_hrn`，但名称查找发生在各自模型中，并受分类、hair code、帽子声明
和角色状态约束。例如 `c0801f0001_fac.mdl` 的 `atr_top`、`atr_mim` 分别
是表下标 3、8，`c1401f0001_fac.mdl` 的 `atr_hrn` 是下标 4。
独立 zear/tail 模型也可以没有 attribute 名；整模型启用与 submesh 的
attribute 掩码是不同层次的操作。

## 模型 attribute 与 shape 关联

消费过程按字符串查询实际模型的 attribute/shape 名称。下表给出观察到的
selector 分支所引用的名字；触发条件包含启用与比较条件：

| Selector | Hand 分支引用 | Foot 分支引用 |
| --- | --- | --- |
| 0 | 该分支不修改所列属性/shape | 该分支不修改所列属性/shape |
| 1 | shape `shp_hij` | shape `shp_sne` |
| 2 | attribute `atr_hij`，shape `shp_ude` | attribute `atr_sne`，shape `shp_hiz` |
| 3 | attributes `atr_hij`、`atr_ude`，shape `shp_kat` | attributes `atr_sne`、`atr_hiz`，shape `shp_mom` |

在这些分支中，attribute 操作清除对应模型掩码位，shape 操作设置其独立掩码位。
Head neck 分支另引用 `atr_nek` 和 `shp_nek`，受 head enabled、neck/gorget、
种族帽子声明与角色状态共同约束。

attribute 的位号来自 **本 MDL 的名称表下标**；同一个名称在不同 MDL 中可
对应不同位。`c0101e0001_top.mdl` 的 `atr_ude` 为第 2 项，
`c0101e0600_top.mdl` 的同名项为第 0 项。Submesh 的 u32 attribute mask 使用
这一文件内索引空间。shape 名表及 shape 掩码是另一索引空间。

上述关系不包含全部耳、尾、头发、NPC 或其他角色状态的消费分支。
