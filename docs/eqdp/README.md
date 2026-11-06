#import "../_components/pele/lib.notc" as pele;

# EQDP 装备变形参数

EQDP 按装备或配饰模型编号保存特定 race/sex 的槽位标志。两类资源分别使用：

- `chara/xls/charadb/equipmentdeformerparameter/c####.eqdp`
- `chara/xls/charadb/accessorydeformerparameter/c####.eqdp`

race/sex 和领域来自路径，文件内不包含种族编号或区分装备/配饰的标记。
两类资源共用以下 Win32 `0x0101` 布局，小端字节序。

## 头与稀疏块

位置以 bit 为单位，原点为文件起点：

#pele.diagram(r#"""
packet
0-15: "Version: 0x0101"
16-31: "Entries per block B"
32-47: "Logical block count N"
"""#)

6 字节头之后是 `u16[N]` 块索引，再后为 `u16` 条目载荷。
块索引值 `0xffff` 表示全零缺省块；其他值是载荷中首条目的索引，单位为 u16 条目，
不是字节。载荷起点为文件偏移 `6 + 2N`。

模型编号 `i` 的查找：

```text
logical_block = floor(i / B)
within_block  = i mod B
p = block_index[logical_block]
value = 0                                      if p == 0xffff
value = payload_u16[p + within_block]           otherwise
```

逻辑地址空间为 `B × N` 条目，缺省块与地址空间之外的编号应区分。
文件末尾可含未被任何块引用的填充。Win32 c0101/c0201 样本中 `B=64`、`N=157`；
块编号并非直接的连续文件偏移，缺省区之后会再次出现非空块。

## 条目位域

位号以读取后 u16 的最低有效位为 0，与文件的小端字节序分别定义。
装备文件的槽顺序为 head、body、hands、legs、feet，每槽两位；高六位保留。
模型/材质标志含义是路径存在相关观察支持的推断，不等同于指定路径一定存在，
也不能由文件单独确定 fallback 路由。

#pele.diagram(r#"""
packet
0-1: "Head flags"
2-3: "Body flags"
4-5: "Hands flags"
6-7: "Legs flags"
8-9: "Feet flags"
10-15: "Unknown"
"""#)

每对低位推断为 material 声明，高位推断为 model 声明。
原始 16 位值和原始两位值是确定的文件数据；标志不能代替实际资源查询。

## 配饰槽与材质引用

配饰条目同样长两字节，其逻辑编号对应 accessory 的 `a####`。数值位号仍以
读取后 u16 的最低有效位为 0；图中位置单位为 bit、原点为条目起点：

#pele.diagram(r#"""
packet
0-1: "Ears: ear"
2-3: "Neck: nek"
4-5: "Wrists: wrs"
6-7: "Right ring: rir"
8-9: "Left ring: ril"
10-15: "Unknown"
"""#)

相应 MDL 路径形如 `chara/accessory/a####/model/c####a####_{slot}.mdl`。
实际 ring 模型的骨名区分 `j_hito_a_r` 与 `j_hito_a_l`；左右槽位仍各占两位。
材质是 MDL 的独立字符串引用，不能仅把模型文件名的 `.mdl` 换成 `.mtrl`：
例如 `c0101a0001_ril.mdl` 的左戒指槽值为 2，其材质名却是
`/mt_c0101a0001_rir_a.mtrl`；`c0201a0001_nek.mdl` 的 neck 槽也为 2，
材质名为 `/mt_c0101a0001_nek_a.mtrl`。

这些字节关系表明模型和材质声明可以分开，材质可共享另一个槽位或 race
命名。低位为 1 也不保证同 race 的模型存在：c0101 的 a0139/a0157 耳饰槽值
均为 1，直接命名的 MTRL 可定位，而同编号同 race MDL 不在 base index 中。
这些观察不规定完整的客户端候选选择与回退优先级。
