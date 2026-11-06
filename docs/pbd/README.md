#import "../_components/pele/lib.notc" as pele;

# PBD 骨骼变形数据

PBD 按 skeleton ID 保存骨骼名与仿射变形矩阵，并用独立的连接表表达父子关系。
角色的 `chara/xls/boneDeformer/human.pbd` 包含种族与性别相关的数据；它的骨骼
系数与骨架 rest TRS 是不同的资源。以下为 Win32 小端布局。

## 主表

文件先有 `u32` 变形器数量 N，随后为 N 个 12 字节变形器记录、N 个 8 字节连接
记录和变形器载荷。变形器表与连接表的排列顺序可以不同，必须通过索引关联。

以下单位为 bit，原点为单个变形器记录起点：

#pele.diagram(r#"""
packet
0-15: "Skeleton ID"
16-31: "Connection table index"
32-63: "Payload file offset (0 = absent)"
64-95: "Skeleton scalar: f32"
"""#)

`Payload file offset` 是文件绝对字节偏移。零表示没有矩阵载荷，不等于丢失文件或
解码失败。`Skeleton scalar` 是按 skeleton ID 保存的 IEEE 754 binary32 标量，
也见称为 unknown scale。它独立于矩阵载荷：没有载荷的记录仍可声明该值。

Win64 `2026.10.06.0000.0000` 的一个静态消费点分别查询角色 skeleton ID 和
所选模型 skeleton ID 的标量，将前者除以后者，保存为该模型槽的独立参数。
这个关系不确定标量的物理量，也不把它定义为需要乘进每个骨矩阵的系数。
主表记录 101 没有矩阵载荷，其标量仍为 `1.5258699655532837`。

连接记录使用有符号 16 位索引，`-1` 表示没有对应链接。位置以 bit 为单位，原点
为单个连接记录起点：

#pele.diagram(r#"""
packet
0-15: "Parent connection index"
16-31: "First child connection index"
32-47: "Next sibling connection index"
48-63: "Deformer table index"
"""#)

parent/child/sibling 索引指向连接表，最后一项指向变形器表。变形器中的连接索引与
连接中的变形器索引互为逆映射。连接组成森林；同一 sibling 链的节点共享 parent。
例如主表 skeleton 202 的连接索引是 20，而不是它在变形器表中的索引 5。
指定 Win64 消费过程分别检查直接 parent 与严格 ancestor 关系；它们从变形器
记录的 connection index 进入连接表，经 parent 和 deformer index 返回名称空间。
parent 关系中的方向不单独确定矩阵的应用方向或坐标空间。

## 骨骼载荷

载荷开头为 `u32` 骨骼数量 B，接着是 B 个 `u16` 骨名偏移，之后补齐到文件的
4 字节边界，再存 B 个 48 字节矩阵，最后为 NUL 结尾的骨骼名字节串。
骨名偏移相对于该变形器载荷起点，单位为字节。样本中的名字为 ASCII；字节串
与 UTF-8 解码可分别处理。

#pele.diagram(r#"""
flowchart TB
    count[u32 bone count B] --> offsets[B x u16 payload-relative name offsets]
    offsets --> padding[Align to 4 bytes]
    padding --> matrices[B x 48-byte affine matrices]
    matrices --> names[NUL-terminated bone names]
"""#)

矩阵为三行、每行四个 IEEE 754 binary32 系数，按行存放。下图单位为 bit，原点
为矩阵记录起点；所有 f32 均为小端：

#pele.diagram(r#"""
packet
0-31: "m00"
32-63: "m01"
64-95: "m02"
96-127: "tx"
128-159: "m10"
160-191: "m11"
192-223: "m12"
224-255: "ty"
256-287: "m20"
288-319: "m21"
320-351: "m22"
352-383: "tz"
"""#)

齐次第四行为 `(0,0,0,1)`。以列向量解释时，平移项为每行第四个系数，
`p' = M × (x,y,z,1)`。转为列主序 4×4 仅重排系数，不应先分解为 TRS 再重建，
否则可能损失剪切等文件数据。跨种族的路径选择和绑定空间转换需结合骨架与
消费分支；下节给出一个祖先链分支的组合约定。

### 祖先链分支的组合关系

Win64 `2026.10.06.0000.0000` 的一个骨名查询分支接受两个 skeleton ID，
要求第二个 ID 是第一个的严格祖先。它从第一个变形器的载荷开始，沿 parent
连接向上取同名骨矩阵，在第二个 ID 的记录之前停止。例如连接链为
`C → P → A`，查询 `C, A` 时，使用 `C` 和 `P` 的矩阵，不包含 `A` 的载荷：

#pele.diagram(r#"""
flowchart LR
    c[Record C: D_C for bone name] --> p[Parent record P: D_P for same name]
    p --> a[Ancestor A: stopping record, excluded]
    c --> product[Affine product D_C x D_P]
    p --> product
"""#)

按上述列向量与行存储约定，组合结果为 `D_C × D_P`；一般写作从起点到
祖先前一项的有序乘积。两项一般不可交换。分支中找不到骨名会返回查询失败，
不能把失败等价为单位矩阵成功。直接 parent 分支则只返回第一份记录的同名
骨矩阵。路径中的记录 ID 与连接表位置仍须通过双向索引转换。

一个模型消费点另将所得 `D` 与输入的列主序矩阵 `G` 组合，输出行存储
仿射矩阵 `G × D`。这是具体分支的乘法约定；`G` 的来源还经过骨架、姿态和
条件修正路径，不能据此把所有 `D` 统一描述为 world-space 或 local-rest TRS，
也不能推定 EPBD 的路径选择或逆向转换规则。

## EPBD 扩展

PBD 可在主数据之后附加 PACK 封装。EPBD 类型的 payload 是带 `EPBD` 文件标识的
FlatBuffers。以下单位为 bit，原点为 PACK 内容头起点：

#pele.diagram(r#"""
packet
0-31: "Type: EPBD"
32-47: "Version: 1"
48-63: "Pack count: 0"
64-127: "Payload byte length: u64"
"""#)

文件末尾为 24 字节 footer，原点为 footer 起点，单位为 bit：

#pele.diagram(r#"""
packet
0-31: "Type: PACK"
32-47: "Version: 1"
48-63: "Pack count: 1"
64-127: "Signed back offset from footer to EPBD header"
128-191: "Total extension byte length: u64"
"""#)

单扩展变体中 `total = 16 + payload_length + 24`，back offset 为
`-(16 + payload_length)`。FlatBuffers 的表使用相对 uoffset、signed vtable
回指及向量长度，不能把表字段当作固定的内联 C 结构偏移。

根表的字段如下。字段序号是 FlatBuffers schema 的逻辑编号，表中实际偏移
由 vtable 给出，不是固定文件偏移；各标量与向量元素采用小端。

| 根字段序号 | 类型 | 内容 |
| --- | --- | --- |
| 0 | 可选 u16 | version tag；指定 Win64 加载器接受省略或显式 1 |
| 1 | 可选 u16 | 意义未知的标量 |
| 2 | entry[] | entry 表的相对偏移向量 |

以下图表示字段关系，不使用字节/bit 坐标。标量字段与 entries 是根表的并列字段：

#pele.diagram(r#"""
flowchart TB
    root[EPBD root table] --> version[Field 0: optional u16 version tag]
    root --> unknown[Field 1: optional u16 scalar]
    root --> entries[Field 2: vector of entry tables]
    entries --> selectors[Entry field 2: u32 local selectors]
    entries --> embedded[Entry field 3: embedded PBD bytes]
    selectors --> ids[Low 16 bits: embedded skeleton IDs]
    embedded --> ids
"""#)

每个 entry 含：

| 字段序号 | 类型 | 内容 |
| --- | --- | --- |
| 0 | u32 | child race code |
| 1 | u32 | parent race code |
| 2 | u32[] | selector 数组；指定 Win64 路径将每个元素低 16 位用作内嵌 PBD skeleton ID |
| 3 | byte[] | 独立内嵌 PBD |

内嵌 PBD 的偏移以该向量中的 PBD 起点为原点，其 skeleton ID 可以是 1、2 等
局部编号，不能直接替换成外层 race code。Win32 样本包含 1501→901、1601→201、
1701→101、1801→201 四个 entry。其 selector 列表分别为 `[1]`、`[1]`、`[1]`、
`[1,2,3,4,6]`；这些局部 ID 全部在对应内嵌表中存在，且其祖先链包含 entry
的 parent race code。selector 是编号列表，不按位组合成条件掩码。

该 Win64 加载路径用外层 child/parent 的低 16 位和局部 selector 的低 16 位
建立三字段查询键，指向对应内嵌表。wire 中的 u32 与读取者使用的 u16 必须
区分：高位字节仍是原始资源的一部分，不应在重存储时丢弃。这一关联不定义
外观状态如何生成 selector，也不等同于完整 EPBD 优先级、回退或矩阵空间。

### 外层 ID 的消费方向

指定 Win64 角色模型槽的一条消费路径，将角色 skeleton ID 放在外层查询的
第一个位置，将所选模型记录的 skeleton ID 放在第二个位置；selector 来自
同一所选模型记录的另一个字段。它们在 EPBD 三字段键中对应 child、parent、
局部 selector。主表严格祖先检查也以第一个 ID 为 descendant、第二个为
ancestor。因此这条路径的 child 是角色侧，parent 是模型侧，不能把两个 ID
对调后仍视作相同查询。

这说明该消费点的角色/模型关联，不把任意 ID 对的逆序查询定义为矩阵求逆。
反向、同 ID、特殊槽与回退选择须由相应消费分支分别解释；world/local/bind
空间仍不能仅凭两个端点的名字确定。文件表的原始 child/parent 仍是 u32，
与该客户端键使用其低 u16 的行为分开描述。
