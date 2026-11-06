#import "../_components/pele/lib.notc" as pele;

# Volume 缺口的存储与生成证据

## 对象与证据类型

对象为 Win32 安装 `2026.10.06.0000.0000` 的 273 条短 BGRA8 volume，
基础尺寸 8×32×24/32、六级 mip，另以 `sky_387.tex` 作完整对照。样本身份
沿用 [Volume 单 mip 证据](volume-mips.md)。短资源的逻辑差额合计 603624
字节；六级完整重建数为 0。完整 base mip 与其他完整组的像素验收独立成立。

证据分别为磁盘字节及解压结果、固定 executable 的指令数据流、在明示
输入条件下的数学推导。没有客户端运行、上传快照或实际 draw 观察。
`ffxiv_dx11.exe` 长 51910400 字节，SHA-256：
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`。
native 地址为首选 VA，image base `0x140000000`；对象偏移单位为字节。
反汇编使用 GNU objdump 2.46，REA 原生引擎可用性边界沿用来源总表。

原始资源、反汇编与大型报告登记的仓库外临时路径为：

- `/tmp/opencode/volume-storage-semantics-01/`：物理区间、逐块字段和读取审计；
- `/tmp/opencode/volume-native-generation-01/`：创建/复制数据流、原始代码
  字节、API 资料、条件性布局和物理/数值交叉核对；
- `/tmp/opencode/volume-mip-lineage-01/`：全组/切片摘要、滤波模型、量化反例
  及四个完整 depth=2 对照的固定捕获。

这四组临时研究目录（另含 `volume-resolution-01`）的路径检查结果为缺失；
报告摘要与结论保留在本页和固定 JSON 中。持久目录
`../icexiv-data-evidence/volume-generator-01/` 保存 `sky_441/067/104/358`
四条身份相同的恢复捕获、两个量化反例复核及 D3DX 9 API 来源。
四条捕获不代表完整总体或临时报告集合已经恢复。

`../icexiv-data-evidence/volume-generator-02/` 保存已审 MIT 原源码的 CPU
对照、两个反例的数值探针及许可记录，见
[生成候选的 CPU 数值证据](volume-generator-oracle.md)。

`../icexiv-data-evidence/volume-generator-03/` 保存正向天空目录的 452 条
捕获、直接 base 张量的总体比较与源域反例；另有
`volume-generator-native-03/` 的 DLL 调用身份调查。恢复的捕获与这些报告
分别有固定摘要，不替代缺失临时报告的历史身份。

## 物理存储的完整区间

只读取已知 `010000.win32.index` 的头和主表，以同 DAT 中严格大于条目起点
的最小索引位置作为后继边界。273 条路径均核对目录/文件名 JAMCRC 与自身
locator，并读取后继的 24 字节条目前缀。主表含 77434 条记录，读取长
1238944 字节；没有扩展到其他包或重复全安装库存。

每条从起点到后继的所有字节均被分类。273 条自身分配区共 1239936 字节，
其中 923589 字节属于头、表、块头和存储载荷；其余 **316347 字节实读全零**。

| 未消费区 | 字节 | 非零字节 |
| --- | ---: | ---: |
| 组表/u16 跨度表之后的头部剩余 | 26460 | 0 |
| 块有效载荷之后的跨度尾部 | 158767 | 0 |
| 最后跨度之后的 128 字节对齐区 | 13104 | 0 |
| 使用容量之后的保留区 | 118016 | 0 |

全部组物理连续，无组间空洞、额外块头扩展或悬空跨度索引。1338 个
DEFLATE 块均到达流末尾，输入字节恰好耗尽，输出等于块头和组声明；720 个
raw 块长度亦精确吻合。这些是字节级流耗尽检查，不解释末个编码字节的
未使用 bit。

### 容量字段的样本关系

令 `H` 为条目头字节长，`P` 为最后跨度的末尾（相对条目起点），`N` 为
后继索引位置减条目起点；字段按小端读取。定义：

```text
U = H + 128 × u32(entry + 0x10)
A = H + 128 × u32(entry + 0x0c)
```

273/273 均满足 `128 × u32(+0x10) = ceil128(P-H)`、`U-P=48` 及
`P ≤ U ≤ A ≤ N`。62 条的 `A>U`，保留区合计 118016 字节；其余 211 条
两值相等。271 条 `A=N`。这里支持的含义是固定样本中的条目体使用容量与
包含保留空间的分配容量，不能据此为其他版本/类型规定字段用途。

| 见证 | P | U | A | N |
| --- | ---: | ---: | ---: | ---: |
| `sky_441` | 1360 | 1408 | 1408 | 1408 |
| `sky_045` | 9424 | 9472 | 10112 | 10112 |
| 完整 `sky_387` | 1104 | 1152 | 1152 | 1152 |

`sky_205` 与 `sky_225` 的 `A<N`，中间分别有一条和三条 8064 字节记录，
每条首六个 u32 为 `[128,0,0,62,0,0]`，其余字节全零；
`128+62×128=8064`，逐条推进恰好到后继。结构与空闲/保留槽相符，type=0
的正式名称、形成历史和删除语义未知。

因此精确检查范围内没有额外已编码 mip 块或尾随流。物理零余量没有映射到
逻辑 mip 缺口的证据，不能据此填零。其他 DAT 区域、包或版本不在此结论内。

## Texture3D 的创建契约

### 标志来源与描述参数

`140305240` 在 `1403052c0..1403052ce` 提取 handle `+0x18` 的 bit 1，
得到 0/1 mode；经 `140305bd0 → 1403064d0 → 1402247b0` 构建 volume。
固定样本的 attributes 为 `0x01000000`，宽高为 8×32；构建后的 texture
`+0x5c` 分别为 `0x05000804` / `0x05001004`。构造器 `14021a902`
将别名字段 `+0xc0` 清零。

[天空请求与缓存模式](volume-sky-consumer.md)通过注册 key、factory 虚表和
具名 TextureResourceHandle，将新建请求的 `+8=3` 与 handle bit 1 关联。
已有 handle 的覆盖条件不同；缓存复制长度使用未右移深度，源推进使用缩减
深度，二者的原指令与条件分别保留。

对该调用链、非空初始像素和未改写 flags 的状态，
`140218d2f..140218d68` 得到 Usage=1；`flags & 0x2001` 与
`flags & 0x40` 的覆盖分支均不触发。`140218fdf..140219051` 写入以下描述：

| D3D11_TEXTURE3D_DESC 字段 | 条件性值 |
| --- | --- |
| Width / Height / Depth | 完整基础级路径为 8 / 32 / 24 或 32 |
| MipLevels | 对象的原始级数；完整路径为 6 |
| Format | 87，B8G8R8A8_UNORM |
| Usage | 1，IMMUTABLE |
| BindFlags | 8，SHADER_RESOURCE |
| CPUAccessFlags | 0 |
| MiscFlags | 0，`14021901c` 明确写零 |

`140219051`调用device虚表`+0x30`，参数按CreateTexture3D合同解释。
[创建输出与条件注册](volume-block-loading.md#创建输出与条件注册)确认
D3D11CreateDevice导入及owner输出地址；创建owner与全局对象的同一性和
所选虚调用实例的来源仍需绑定。273条短头、两种mode的546组推导均符合
上述描述参数，完整对照另核对两组；这些计数不表示API实际调用成功。

Microsoft 的 D3D11 合同规定 IMMUTABLE 资源创建后不能由 CPU/GPU 写入。
GenerateMips 要求 SHADER_RESOURCE、RENDER_TARGET 和
RESOURCE_MISC_GENERATE_MIPS；条件不满足时调用无效。在接口身份与上述
描述成立的条件下，该volume创建路径没有自动生成低mip的条件。该结论
依赖正向描述数据流，不依赖
搜索不到某个虚表槽，也不推广到其他对象或未知的状态改写。

### 像素源与完成路径

BGRA8 跳过三个格式转换分支。`140218711..14021879d` 为每个 mip 写入
源指针、row pitch、slice pitch，源按 `slice_bytes(m) × max(1,D>>m)`
推进。短组没有改变 Texture3D 的深度或提供一个滤波回调。

以下是数据流关系图；节点中的偏移为对象内字节，不表示文件坐标：

#pele.diagram(r#"""
flowchart TB
    groups[SqPack groups] --> staging[Handle +0x148 staging]
    staging --> descriptors[Full-depth subresource descriptors]
    descriptors --> create[Conditional CreateTexture3D: IMMUTABLE, MiscFlags 0]
    create --> install[Release staging; install handle +0x128]
    source[Existing source subresource] --> copy[Command 11: conditional copy parameters]
    copy --> replacement[Replacement texture]
"""#)

`140304800..140304941` 和 `140304a80..140304bda` 条件调用初始化函数，
再释放 staging、将 handle `+0x130` 的 texture 安装到 `+0x128`；这两段没有中间
mip 生成步骤。栈上描述表清零没有定义 staging 缺口的值。分配链的已知
区域子分配边界见 [TEX 来源页](tex.md#volume-上传缓冲区的分配路径)。

## 替换与另一上传路径

完整 `140306d60..140306ff4` 含三个 CHAININFO 关联的 unwind 区域；
只读取首个 `.pdata` 区域会漏掉正文。它以源/目标宽高确定 mip 偏移，
逐级发出 command 11：目标 texture/mip、已有源 texture/mip、目标 x/y=0，
`+0x30=0`、`+0x48=0`。

分派表 `140235258 + 11×4` 指向 `140234dd2`。目的对象+0x5c的bit21
清零、格式相同、mip非负且命令`+0x48=0`时，在`140234e6c`以
`[context+0x17b8]`为接口调用虚槽`+0x170`。发布字段对应目标x/y/z=0、
source box为NULL。该ABI与CopySubresourceRegion的参数布局相容；
[命令引用证据](volume-block-loading.md#命令引用与条件复制)保留lane可达
与接口来源条件；[创建输出与条件注册](volume-block-loading.md#创建输出与条件注册)
绑定所选TLS写入、设备创建输出和wrapper赋值，但创建owner、全局对象与
batch参数的同一性没有闭合。旧临时报告的接口身份链没有因重捕获恢复。
若接口身份成立，API合同规定复制不缩放、不混合、
不滤波；源subresource的像素仍须另有来源。

`140305e10` 在 flags `&0x1800` 非零时调用同一初始化函数；另一 volume
分支转入 `140307c60..140307e4d`，取得各 mip 缓冲区后 memcpy 完整
row×height×depth，深度逐级右移一位且最小为 1。`140217490..1402174ec`
的条件路径亦尾调用 memcpy。该路径没有从 base mip 计算省略像素。

另外两个 `140216dc0` 直接调用候选均从函数入口确认了指令边界：

- `140350c70..140350d89` 创建 2×2×1、单 mip、格式 `0x1132` 的 volume，
  向栈上 512 字节区域传入填充值 `0xff`，再于 `140350d59` 初始化。
- `14042aca0..14042b729` 在 `14042b6d3` 初始化 8×32、单 mip、格式
  `0x2460` 的色表纹理；源指针来自材质色表/染色或已有纹理的读取，关联见
  [Legacy 色表纹理参数](legacy-color-conversion.md#行扩展与纹理参数)。

这两个调用的尺寸、格式和 mip 数均与六级 BGRA8 天空资源不同。

## 已存像素的三维滤波关系

### 坐标、总体与比较算子

像素比较复用 274 条捕获，另按已知位置读取完整 depth=2 的 `sky_104`、
`sky_111`、`sky_358`、`sky_372`，没有扩大索引库存。273 条短资源包含
177 条不同六组像素链；相同 base 摘要下没有已存组链冲突。
原始组为 BGRA 字节，坐标原点在组首个像素，通道序号 `c=0..3`：

```text
Wm = max(1, 8 >> m), Hm = max(1, 32 >> m)
byte_offset(x,y,z,c) = 4 × ((z × Hm + y) × Wm + x) + c
```

有界比较包含 2D 逐片 box、3D box、point、中心 tent、周期/clamp 边界，
以及 z 前缀、二倍/四倍步长、直接从 base 与逐级量化等明确模型。数值关系
最强的是 **三轴周期分段线性插值的目标单元积分**：

- 一轴原长度为 `N`、逻辑输出长度为 `n=max(1,N>>m)`，令 `s=N/n`。
  这些样本的比值均为整数；`s=1` 时该轴采用 identity。
- 源像素位于 `i+1/2`，对目标单元 `[s*j,s*(j+1)]` 积分并除以 s。
  `s≥2` 的权重为 `[1,7,8,...,8,7,1]/(8*s)`，首个源索引为 `s*j-1`。
  二倍缩小即 `[1,7,7,1]/16`；三轴权重相乘，轴间不量化。
- 索引按各轴原长度取模，在编码字节值上计算，不做 gamma 变换。
  D=24、m=4 时 z 单元覆盖全部 24 层。

这个算子是从样本提出并验证的数学假设。其精确有理数结果记为 F。
273 条全部 **529116 个已存低 mip 字节**均在 `floor(F)..ceil(F)` 内，
按 177 条链去重为 369948 字节。逐级重复同一平滑核存在大于 1 的反例，
不能由这个关系推断逐级执行顺序。

### 切片身份与长度的区别

每个已存片与候选逻辑 mip 的所有 z 位置比较，要求全部通道均落入该位置
的精确 floor/ceil 区间。每片都兼容同序号的前缀位置，重复源层可留下歧义。

| mip | 去重链的已存片数 | 仅兼容前缀位置 | 兼容前缀及其他位置 | 不兼容前缀 |
| --- | ---: | ---: | ---: | ---: |
| 1 | 1346 | 804 | 542 | 0 |
| 2 | 319 | 275 | 44 | 0 |
| 3 | 177 | 156 | 21 | 0 |
| 4 | 177 | 162 | 15 | 0 |
| 5 | 177 | 177 | 0 | 0 |

`sky_281/269` 的 mip 1 六片仅兼容逻辑 z=0…5；`sky_045/219/017` 的
mip 2 两片仅兼容逻辑 z=0、1。这里的 z 是所定义模型的输出索引。
数值足迹的 z 步长在轴未缩至 1 时为 `2^m`，与组长度中的 `D>>(2*m)`
分开。数据强支持正常三维 mip 的前缀保存关系，生产端“先生成完整 mip、
再截取前缀”的写出过程需要代码证据。

以 177 条链的 mip 1 BGR 通道计，3D box 前缀的最大误差为 41；
同一积分 XY 核改成逻辑 z 间隔取片或将 z 直接缩至短组深度，也有大幅反例。
因此片数公式不能用作滤波的深度缩小因子。

### 缩短源域的条件性反例

03 的两种数学假设保持正常 `2^m` 缩小比例，将源 Z 域缩到
`stored_slices × normal_scale`，并在缩短域内分别 wrap 或 clamp。
`sky_045/281` 的 mip 1/2 共 8 个资源/mip/边界组合中，7 个有实际通道
落在裁域积分 floor/ceil 之外：

- `sky_045` mip 1 `(0,0,0)` B：crop-local clamp 给出精确 6，实际为 16；
  完整源域积分为 `65001/4096`。
- 同资源 mip 2 `(0,0,1)` B：crop-local wrap 的区间为 97…98，实际为
  102；完整源域积分为 `3345627/32768`。
- `sky_281` mip 2 `(0,0,0)` B：两种边界均为 73…74，实际为 75；
  完整源域积分为 `38387/512`。

七个反例由独立三角基函数积分复算。`sky_281` mip 1 的 clamp 在首末
已存片的 384 个 BGR 通道中没有此类反例；此项不代表其全域精确匹配。
这些模型没有被认定为 D3DX box 的实际边界语义。

“完整生成后保存前缀”与“访问完整源域、仅计算前缀目标坐标”可在已存域
产生相同结果。源域反例约束的是数学支持域，具体写出过程仍需生产代码。

### Bayer 量化关系及精确反例

以标准 4×4 Bayer 次序建立固定量化假设：

```text
B = [ 0,  8,  2, 10
     12,  4, 14,  6
      3, 11,  1,  9
     15,  7, 13,  5 ]
j = (z + (x if y is even else Wm-1-x)) mod 4
t = (2*B[y mod 4][j]+1)/32
candidate = clamp_u8(round_ties_even(F + 1/2 - t))
```

坐标为该 mip 的输出坐标；奇数行反向 x 的宽度使用实际 Wm。
177 条链的 277461 个已存 BGR 通道中，276980 个相同、481 个相差 1。
其中 480 个差异位于精确阈值，另一个离阈值 `1/4096`。
按 273 条展开为 396837 个 BGR 通道、2919 个差异；重复资源使计数权重不同。

| 反例 | 精确数值与相位 | 实际字节 |
| --- | --- | --- |
| `sky_441` mip 1 `(x,y,z)=(1,0,1)` | F=6373/32，t=5/32 | B/G/R 均为 200 |
| 同资源 `(1,4,1)` | 完整 4×4×4 邻域逐字节相同，F 和 t 相同 | B/G/R 均为 199 |
| `sky_067` mip 5 的 B | F=204927/4096，t=1/32；F 比 50+t 小 1/4096 | 51，候选为 50 |

前两点说明单一精确 tie 舍入规则不能决定所有已存字节；第三点也不能用
更换 tie 规则消除。生产实现的浮点精度、坐标累计、归一化、运算次序或
作者端中间值没有由这组拟合唯一确定。

完整非退化对照 `sky_104/358` 的全部低 mip 逐字节符合该量化模型；
`sky_372` 只有 mip 1 一个阈值字节有差异。全白 `sky_111` 和全零
`sky_387` 为退化控制，不用于区分滤波类型。

独立交叉核对从三角基函数的有理数原函数积分构造系数，直接求三轴张量和，
没有调用模型调查的数值实现；复现 177 条链的 floor/ceil 范围、481 个差异、
两个反例及四个完整对照。数值吻合没有提供缺失坐标上的独立历史像素。

### 生成代码的定位边界

固定 executable 的各 PE 节原始字节中，没有命中上述 Bayer 次序的
u8/u16/u32/f32 连续表、f32 阈值/偏置表或 f32 积分核字面序列。指定
D3DXFilter/LoadVolume/11CreateTexture 函数名亦无字面命中；唯一相关 DLL
字面量为 `D3DX9_43.dll`。这些搜索只覆盖指定编码，不能排除计算得到的
表、其他排列、动态模块或作者工具，也不能将 DLL 名称视作生成调用证据。

`volume-generator-native-03` 将该字面量固定到 VA `0x142110bd0`，但其
加载、解析及调用身份未建立。常规地址引用、普通/延迟导入的定向检查没有
提供入口；邻近正向链使用 `sdologinentry64.dll`，解析
`SDOLInitialize/SDOLGetModule/SDOLTerminal`，不能归属给 D3DX。该检查
不排除计算地址、动态名称或其他模块内部调用。

待确定的关键关联是：生产滤波与量化的具体数值执行语义，以及完整三维
结果如何成为 `D>>(2*m)` 长度的存储前缀。客户端已审创建/复制路径与像素
模型没有形成这条生成证据链。六级完整重建数为 0。

### D3DX 9 候选的 API 契约

Microsoft 文档为生成器定位提供以下合同；它们没有证明 FFXIV 生产端
实际选择了该实现或参数。

| 接口或标志 | 文档定义 |
| --- | --- |
| D3DXLoadVolumeFromMemory / FromVolume | 默认 Filter 为 TRIANGLE \| DITHER |
| D3DX_FILTER_DITHER | 4×4 有序抖动 |
| D3DX_FILTER_MIRROR_U/V/W | 对应轴边界使用镜像代替周期延拓 |
| D3DXFilterTexture | 默认对 power-of-two 为 BOX，否则 BOX \| DITHER；逐级递归生成 |
| D3DXFilterVolumeTexture | D3DXFilterTexture 的宏别名 |

TRIANGLE 与 4×4 有序抖动构成可追踪的 volume 载入/缩放候选。精确积分核、
Bayer 排列、奇数行遍历、z 相位、浮点精度和运算顺序没有在这些文档中定义。
默认递归 BOX 与样本中的普通 box 反例分开，不能因 API 名称包含 mip
filter 就认定其默认行为匹配。

生成器的决定性验证需要有身份的二进制或生产源码及调用参数，先同时解释
`sky_441` 的等邻域反例和 `sky_067` 的非 tie 反例，再核对完整非退化资源。
片数从 `D>>m` 到 `D>>(2*m)` 的关系还需写出长度或源/目标 box 的代码
证据。单独的滤波拟合不能证明“先完整生成再截取前缀”的执行过程。

### 原源码 CPU 探针的约束

[DirectXTex 固定版本对照](volume-generator-oracle.md)为两个反例分别提供
可复现机制：`sky_441` 的周期边界改变源贡献累计顺序，产生 1 ULP 差异，
量化后为 200/199；`sky_067` 从 base 直接累计全体均值的 binary32 误差
越过非 tie 阈值，量化后为 51。四条见证 mip 1 按预先固定的奇数行相位
显式适配后，4096 字节全部相同。

该版本的原始 3D TRIANGLE API 实际递归读取上一 mip，且其 BGRA8 写出不
调用抖动；浮点递归加独立抖动的组合在四条见证的 4528 字节中有 725 个
差异。两个局部机制与相位适配没有形成完整统一生成器，也没有识别生产
调用或前缀写出链。六级完整重建数为 0。

[直接 base 的目录总体](volume-generator-oracle.md#直接-base-张量的目录总体)
对 273 短条目和 179 完整对照的 591408 个已存低 mip 字节，得到
separate/FMA 候选分别 131/113 个差异，其中短条目为 89/79。等候选输入
却有不同实际输出，以及 FMA 的非 tie 反例，分别限制了仅改末端量化或
tie 规则的解释。该总体没有产生缺失坐标的独立历史像素。

04 的[实际精度检查](volume-generator-oracle.md#scalar-与-x87-的实际精度)
区分 GCC 自动向量化产生的 SSE/scalar 混合路径与保留 x87 寄存器累计的
路径。两者在同一 116-byte 门槛中分别有 2/7 个差异，整数有理数复算
确认全部中间位和最终 byte；均未扩展到目录总体。官方历史版本审阅也
没有提供每级直接 base 或奇数行反向相位的生产身份。

### FIIN 元数据入口

`../icexiv-data-evidence/volume-writer-05/` 检查授权安装根目录的
`fileinfo.fiin`。全文件 1120 字节，起始签名为 `FileInfo`，小端 u32
`+0x18/+0x1c` 为 1024/96，与尾部记录的起点和长度对应。
唯一 96 字节记录中的 `nvngx_dlss.dll`、u64 长度 53689400 和 20 字节值，
分别与具名松散文件及其 SHA-1 精确匹配；其余头区、名字余量及四字节尾部
实读全零。单条匹配不扩展为完整 FIIN 规范。

这份文件没有提供天空 TEX 的原始大小、摘要或 writer 信息。固定客户端
中精确 ASCII/UTF-16LE 文件名均无命中，读取入口未建立；该检查不排除
动态名称或安装器消费。DLL 只做有界流式摘要核对，没有加载、执行或读取
实现代码。原件与具名文件的前后身份一致，未重新扫描 DAT/index。

05 封存 14 个文件、22193 字节，清单 SHA-256 为
`5ca0fa93fbcf355fe5482916547ff0ae0d56e272e95e96223cc89b30ade3a2a9`。
原始 FIIN SHA-256 为
`bac708676a5deee88a6697791b0a2754600ed10c305100aeb3c3c6a9d773ac94`；
完整报告与审核身份见 `tests/fixtures/volume-generator-precision-observations.json`。

### 作者输入与天空坐标

[作者画面与采样链](volume-production.md)关联 2019 Photoshop 具名横带、
2026 base 的切片重复，以及表 u16 参数到 shader 第三维坐标的数据流。
147 条 depth32 的 z24…31 与 z0…7 共 1204224 字节完全相等；独立解包
复核 452 条身份。CPU 按分别舍入的运算形成 `N×t/D+0.5/D`，其中 t 来自
输入除以 86400，Sky/Sky2 直接使用结果作第三维采样坐标。
这些证据限定作者输入候选与 base 消费语义，未提供生产 mip 或前缀 writer。

## 来源与复核身份

API 资料来自 MicrosoftDocs/sdk-api commit
`656e9656bfaef34dd29cd7ee9e1146aec17e924c`，作者/发布者为 Microsoft。
读取前复核 CC BY 4.0 LICENSE，SHA-256
`7183ef9749eb9b75e710a81a4c812a9405cc184f087500581a973e5929ac3f5e`。
外部 `sdk-api-pinned.json` 保存 Texture3D 描述、subresource data、Usage、
bind flags、resource misc flags、CreateTexture3D、GenerateMips、CopySubresourceRegion
八篇文档的固定 URL 和摘要；本页是字段合同的中文归纳。

D3DX 9 资料为 MicrosoftDocs/win32 revision
`51c9b7cc54ef393b6406c8320a7840fdd88209d5` 的 `desktop-src/direct3d9/`
下 `d3dx-filter.md`、`d3dxfiltertexture.md`、`d3dxloadvolumefrommemory.md`
和 `d3dxloadvolumefromvolume.md`。先审 CC BY 4.0 LICENSE，摘要与上述
LICENSE 相同。持久目录的 `sources.json` 保存完整固定 URL 与每篇摘要。

| 证据产物 | SHA-256 |
| --- | --- |
| native `creation-report.json` | `204e0ba3880b512d85fa71ca3a8358e8d3eca11075bcb3e08670748c970a6141` |
| storage `derived/conclusions.json` | `3cc9a36d30e0ad442047035390b90123b63d1da088214467a70922cf73ad9d46` |
| native `storage-crosscheck.json` | `8ae37def2b1f69a4ae36bbb8d7bfb955b6bb61e0a7cd689eda02322193651d93` |
| lineage `summary.json` | `2fb0d4589ecfc701f2da59f334023ed9f265528c5264b0daff9f26a6150aa37e` |
| lineage `quantization-counterexamples.json` | `d094dab77b64eb960638221c5f2556f2383e17b8b42215b5452a60753248e125` |
| lineage `control-manifest.json` | `58587f1cf503a49261ab5065df6ecd16d7e056a7f22c7678ecb429807ef365ea` |
| native `lineage-crosscheck.json` | `6be0ff39281454488da92e48df8bbfccc9de621b40a04d3129bef8d7e2366e40` |
| generator `sources.json` | `1d191f2b4f99103cc54074599768f3de1b26710e0875ecf2a027d0191af72e25` |
| generator `witnesses.json` | `05bc5c11712e1dc4efa01b7dff2aeef815df8903fa42b96dacdb3fb1276e8503` |
| generator `witness-checks.json` | `30feb27c020b591b9526c1dc36ba0b7c79902f80617f72dbcffe07142ca8066f` |

`tests/fixtures/volume-generation-observations.json` 固定报告身份与结论边界。

03 的目录、源域与 native 身份固定于
`tests/fixtures/volume-generator-cohort-observations.json`。源域报告
`source-boxes.json` 的 SHA-256 为
`00968c51243a5abc4c4d7d965dae4cf7e6e953dcf4463970a1e3b1fab80e4155`；
native `verified/summary.json` 为
`ca71b146e32e5b0957e69173c6e78493b86e787eb0ace45e7ffb4ef98732800b`。
native 的 53 个封存产物逐摘要核对通过。

`volume-native-generation-01` 报告固定 23 个字节锚点、六段代码与 unwind
身份；布局运算没有执行客户端。物理区间交叉核对只读取已捕获的
1272192 字节，逐字段对照块头，
验证分类范围互不重叠、未分类字节全零及四条 type=0 记录的完整边界。
临时报告的摘要是其历史产物身份，原文件缺失时不能仅由摘要重放。
`volume-generator-01` 捕获脚本只读四个固定 DAT 区间，共 7104 字节，零 index
读取；原始 entry/header 摘要全部一致，两个量化反例亦逐值复现。
脚本采用排他输出、范围预算及前后文件身份检查；固定观察见
`tests/fixtures/volume-generator-candidate-observations.json`。

`volume-mip-lineage-01` 的数值调查使用 Python 标准库和明确的数学模型，
该实验没有查阅额外外部实现。DirectXTex CPU 对照的来源与接触范围单列于
[生成候选来源](volume-generator-oracle.md#来源-许可与阅读范围)。
`lineage-crosscheck.json` 的输出只包含已存位置的误差、摘要与反例，
没有导出候选缺失像素或修复 TEX。

没有引入 FFXIV 实现依赖或读取 `legacy/`。既有外部实现接触与许可证记录
完整保留，不宣称严格 clean-room。完整重建需要可独立核对的缺失内容或
生产生成规则；静态、物理与数值证据没有给出精确重建规则。
