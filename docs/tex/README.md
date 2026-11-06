#import "../_components/pele/lib.notc" as pele;

# TEX 纹理格式

FFXIV 的 `.tex` 文件保存纹理尺寸、像素格式、mipmap 索引和图像数据。图标、
界面贴图和模型材质贴图都使用 TEX。以下描述 Win32 平台的小端布局。

## 文件结构

```text
TEX 文件
├── 文件头（0x50 字节）
│   ├── 属性与像素格式
│   ├── 尺寸与 mip 数量
│   ├── LOD 索引
│   └── mip 数据偏移表
└── 图像数据
    ├── mip 0
    ├── mip 1
    └── …
```

普通线性 TEX 的 mip 位置由文件头中的偏移表给出，偏移相对于文件起点。
Win32 volume 实样本还包含全部有效 offset 为 80、负载按尺寸连续排列的头部，
其结构见下文的 volume 偏移变体。

## 文件头

文件头长度为 `0x50`（80）字节，无文件签名字节。整数使用小端字节序。

图中位置以 **bit** 为单位，相对于文件起点：

#pele.diagram(r#"""
packet
0-31: "Attributes"
32-63: "Format"
64-79: "Width"
80-95: "Height"
96-111: "Depth"
112-119: "MipFlags"
120-127: "ArraySize"
128-159: "LodIndices[0]: High"
160-191: "LodIndices[1]: Medium"
192-223: "LodIndices[2]: Low"
224-255: "SurfaceOffsets[0]"
256-287: "SurfaceOffsets[1]"
288-319: "SurfaceOffsets[2]"
320-351: "SurfaceOffsets[3]"
352-383: "SurfaceOffsets[4]"
384-415: "SurfaceOffsets[5]"
416-447: "SurfaceOffsets[6]"
448-479: "SurfaceOffsets[7]"
480-511: "SurfaceOffsets[8]"
512-543: "SurfaceOffsets[9]"
544-575: "SurfaceOffsets[10]"
576-607: "SurfaceOffsets[11]"
608-639: "SurfaceOffsets[12]"
"""#)

| 偏移 | 类型 | 字段 | 含义 |
| --- | --- | --- | --- |
| `0x00` | `u32` | Attributes | 资源属性、纹理类型及存储标志 |
| `0x04` | `u32` | Format | 像素格式编号 |
| `0x08` | `u16` | Width | 基础级宽度 |
| `0x0a` | `u16` | Height | 基础级高度 |
| `0x0c` | `u16` | Depth | 体纹理的基础级深度；普通二维纹理为 1 |
| `0x0e` | `u8` | MipFlags | 低 7 位为 mip 数量，高位含义未明 |
| `0x0f` | `u8` | ArraySize | 二维数组元素数；cube 样本取 0 或 6，均表示六面；普通 2D、volume 为 0 |
| `0x10` | `u32[3]` | LodIndices | 高、中、低 LOD 对应的 mip 索引 |
| `0x1c` | `u32[13]` | SurfaceOffsets | 普通布局的 mip 文件绝对字节偏移；volume 有全部有效项为 80 的变体 |

### 纹理类型

`Attributes & 0x13c00000` 提取纹理类型：

| 值 | 类型 | 图像组织 |
| --- | --- | --- |
| `0x00400000` | 1D | 一维纹理 |
| `0x00800000` | 2D | 单张二维图像 |
| `0x01000000` | 3D | 随 mip 缩小宽、高、深度的体纹理 |
| `0x02000000` | Cube | 六个面的立方体贴图 |
| `0x10000000` | 2DArray | 多张同尺寸二维图像组成的数组 |

立方体每级包含六个面；二维数组每级包含 `ArraySize` 张图像；体纹理每级包含
该级深度对应的切片。数组元素数和立方体面数不随 mip 缩小。

Win32 样本按 mip 优先组织：同一 mip 的所有层、面或深度切片连续存放，随后才是下一 mip。
每张二维图像单独计算压缩块向上取整后的字节数；不能先合并高度再计算块数。
例如 256 × 256 BC7 cube 的 mip 1 偏移为 `80 + 6 × 65536 = 393296`。
指定 Win32 Direct3D 初始化路径将六个文件面按原 ordinal 交给六个 array
slice，其面序对应 `+X,-X,+Y,-Y,+Z,-Z`。这命名的是 cube 资源的六个面，
不赋予环境图世界空间方向，也不规定应用侧的坐标变换。

#pele.diagram(r#"""
flowchart LR
    header[80-byte header] --> m0[Mip 0: all layers / faces / z slices]
    m0 --> m1[Mip 1: all layers / faces / z slices]
    m1 --> mn[Other mips]
"""#)

### Cube 面与 subresource 编号

同一 mip 内的 face 编号从 0 到 5，依次为 `+X,-X,+Y,-Y,+Z,-Z`。
面内二维像素保持文件行顺序。Direct3D 的 subresource 编号是
`mip + face × mip_count`；它与 TEX 的 mip-major 文件组织不同。
指定初始化路径通过描述表关联这两种顺序，不要求把 TEX 文件改写为
face-major，也不交换 face ordinal。

#pele.diagram(r#"""
flowchart TB
    file[File order: mip m then faces 0 through 5] --> face[Image at file face f]
    face --> sub[Descriptor index: m + f times mip count]
    sub --> cube[D3D cube faces: +X -X +Y -Y +Z -Z]
"""#)

这里的编号与 mip/face 为逻辑索引，不是字节或 bit 偏移。六个面不表示六个
cube；`ArraySize=0/6` 的两个实样本编码均对应单个 cube。其他 array-size
编码是否表示 cube array 需相应变体证据，不能直接把该字节除以六推广。
面名也不单独确定采样向量到面内 UV 的公式、像素翻转或世界坐标约定。

### 属性标志

属性位名称沿用格式参考中的命名。标志名称表达资源用途，不足以单独确定所有平台
的像素寻址算法。

| 位值 | 名称 |
| --- | --- |
| `0x00000001` | DiscardPerFrame |
| `0x00000002` | DiscardPerMap |
| `0x00000004` | Managed |
| `0x00000008` | UserManaged |
| `0x00000010` | CpuRead |
| `0x00000020` | LocationMain |
| `0x00000040` | NoGpuRead |
| `0x00000080` | AlignedSize |
| `0x00000100` | EdgeCulling |
| `0x00000200` | LocationOnion |
| `0x00000400` | ReadWrite |
| `0x00000800` | Immutable |
| `0x00100000` | TextureRenderTarget |
| `0x00200000` | TextureDepthStencil |
| `0x04000000` | TextureSwizzle |
| `0x08000000` | TextureNoTiled |
| `0x80000000` | TextureNoSwizzle |

## 像素格式

格式编号描述一个像素或一个压缩块的编码方式。下表列出参考资料中的格式枚举；
枚举中的名称不表示每种资源或客户端版本都会使用该格式。

### 整数格式

| 编号 | 名称 | 每像素字节数 | 编码 |
| --- | --- | --- | --- |
| `0x1130` | L8 | 1 | 8 位亮度 |
| `0x1131` | A8 | 1 | 8 位 alpha |
| `0x1440` | B4G4R4A4 | 2 | B、G、R、A 各 4 位 |
| `0x1441` | B5G5R5A1 | 2 | B、G、R 各 5 位，A 为 1 位 |
| `0x1450` | B8G8R8A8 | 4 | B、G、R、A 各 8 位 |
| `0x1451` | B8G8R8X8 | 4 | B、G、R 各 8 位，X 为填充 |

字节对齐的通道按名称顺序存储。例如 B8G8R8A8 的四个字节依次是 B、G、R、A。
打包格式名称中的第一个通道占最低有效位：

```text
B4G4R4A4（u16）
  bits  0..3  = B
  bits  4..7  = G
  bits  8..11 = R
  bits 12..15 = A

B5G5R5A1（u16）
  bits  0..4  = B
  bits  5..9  = G
  bits 10..14 = R
  bit  15     = A
```

### 浮点格式

| 编号 | 名称 | 每像素字节数 | 编码 |
| --- | --- | --- | --- |
| `0x2140` | R16F | 2 | 一个 binary16 通道 |
| `0x2150` | R32F | 4 | 一个 32 位浮点通道 |
| `0x2250` | R16G16F | 4 | 两个 16 位浮点通道 |
| `0x2260` | R32G32F | 8 | 两个 32 位浮点通道 |
| `0x2350` | R11G11B10F | 4 | 一个 u32 内的三个无符号浮点通道 |
| `0x2460` | R16G16B16A16F | 8 | 四个 16 位浮点通道 |
| `0x2470` | R32G32B32A32F | 16 | 四个 32 位浮点通道 |

R16F 使用 IEEE 754 binary16：一个符号位、五个指数位、十个尾数位，指数偏置
15。R11G11B10F 以小端 u32 存放；以下位置为该整数的数值位编号，低位为 0，
原点是单个像素，坐标单位为 bit。数值位编号与四字节的存储顺序分别定义。

#pele.diagram(r#"""
---
config:
  packet:
    showBits: true
    bitsPerRow: 32
    bitOrder: ascending
---
packet
0-5: "R fraction"
6-10: "R exponent"
11-16: "G fraction"
17-21: "G exponent"
22-26: "B fraction"
27-31: "B exponent"
"""#)

三个通道各有五个指数位、偏置 15，没有符号位；R/G 尾数各六位，B 尾数五位，
不共享指数。对尾数位数 `n`、尾数整数 `m` 和指数 `e`：`e=0` 时值为
`m × 2^(-14-n)`，`1≤e≤30` 时值为 `(1+m/2^n) × 2^(e-15)`；
`e=31,m=0` 表示正无穷，`e=31,m≠0` 表示 NaN。R/G 最小非零值为
`2^-20`、最大有限值 65024，B 对应为 `2^-19` 和 64512。

### 块压缩格式

每个块覆盖 `4 × 4` 个像素。即使某一级的宽或高小于 4，也为该方向保留完整的块。

| 编号 | 名称 | 别名 | 每块字节数 | 通道内容 |
| --- | --- | --- | --- | --- |
| `0x3420` | BC1 | DXT1 | 8 | RGB，或带 1 位透明度的 RGB |
| `0x3430` | BC2 | DXT3 | 16 | RGB 与逐像素 4 位 alpha |
| `0x3431` | BC3 | DXT5 | 16 | RGB 与插值 alpha |
| `0x6120` | BC4 | — | 8 | 单通道 |
| `0x6230` | BC5 | ATI2 | 16 | 双通道 |
| `0x6330` | BC6H_SF16 | BC6H | 16 | 有符号 HDR RGB；指定 Win64 映射为 DXGI 96 |
| `0x6432` | BC7 | — | 16 | RGB / RGBA，多种块编码模式 |

#### BC6H 的端点与索引

BC6H 每个 128 位块编码 4×4 个 RGB 像素，没有 alpha；消费时 alpha 为 1。
它有 UF16（无符号）与 SF16（有符号）变体，二者的端点符号解释不同。
Win64 `2026.10.06.0000.0000` 的 TEX `0x6330` 对应 SF16。它保留负值与
大于 1 的 HDR 值，不能把解码像素等同于已钳制的八位颜色。

每块的模式决定端点精度、是否使用差值端点、单区域或双区域，以及索引位宽。
共有 14 种模式；双区域模式另有五位 partition ID，在 32 种分区中选择，
每区域独立插值其端点。每个区域的 anchor 像素索引省略最高位。

以官方 mode 11（单区域，两组直接有符号 10 位端点）为例，布局如下。
单位为 bit，原点是单个 16 字节压缩块的最低有效位，按字节内低位到高位编号；
多字节字段低位先存，位编号与文件中多字节数的小端字节序分别定义：

#pele.diagram(r#"""
packet
0-4: "Mode bits = 00011"
5-34: "Endpoint 0: signed R10, G10, B10"
35-64: "Endpoint 1: signed R10, G10, B10"
65-67: "Pixel 0 anchor index: 3 bits"
68-127: "Pixels 1..15: 4-bit indices"
"""#)

索引按 4×4 块内逐行顺序排列。四位索引的权重依次为
`0,4,9,13,17,21,26,30,34,38,43,47,51,55,60,64`，用于端点精度展开后的
整数调色板插值，再缩放为 binary16 位模式。精度展开、混合、最终 half
转换的顺序影响结果，不能先把量化端点当成普通 f32 直接线性插值。

保留的五位模式码 `10011/10111/11011/11111` 规定输出 RGB 全零、alpha=1。
其他有效模式的端点布局与该示例不同，不能套用 mode 11 的固定字段位置。

### 深度与特殊格式

| 编号 | 名称 | 含义 |
| --- | --- | --- |
| `0x0000` | Unknown | 未指定格式 |
| `0x4140` | D16 | 16 位深度 |
| `0x4250` | D24S8 | 24 位深度与 8 位模板 |
| `0x5100` | Null | 空格式 |
| `0x5140` | Shadow16 | 阴影深度格式 |
| `0x5150` | Shadow24 | 阴影深度格式 |

参考枚举另保留 `0x1452`，别名为 `A8R8G8B82`；其具体编码缺少说明，不据名称
推导通道布局。

## Mipmap 与数据大小

mip 0 为基础图像，后续级别逐级缩小。对于级别 `i`：

```text
width_i  = max(1, Width  >> i)
height_i = max(1, Height >> i)
depth_i  = max(1, Depth  >> i)    // 体纹理
```

偏移表容纳 13 级。`LodIndices` 的值指向其中某一级，例如索引 2 表示 mip 2，
该级数据的位置由 `SurfaceOffsets[2]` 给出。

对于线性存储的单张二维图像，编码数据大小为：

```text
整数 / 浮点格式：
  size_i = width_i × height_i × bytes_per_pixel

BC 格式：
  blocks_x = ceil(width_i / 4)
  blocks_y = ceil(height_i / 4)
  size_i   = blocks_x × blocks_y × bytes_per_block
```

例如，BC1 的 `8 × 8`、`4 × 4`、`2 × 2`、`1 × 1` 四级分别占 32、8、8、8
字节。小尺寸级别的块中包含超出图像宽高的像素位置，这些位置属于块填充。

线性图像按行排列，BC 数据按块行排列；swizzle / tiled 布局还涉及像素或块的
地址重排。普通 offset 布局每级起点以偏移表为准，不能仅靠前一级的理论大小推算。

### Volume 偏移变体

Win32 `2026.10.06.0000.0000` 的部分 volume 文件，全部有效 SurfaceOffsets
项均为 `80`。存在完整数据的实例是 `8×32×2` BGRA8、六级 mip：压缩组按 mip
顺序提供 `[2048,256,64,16,8,4]` 字节，头后连续排列，整文件长 2476 字节。
这类项值不分别给出各 mip 的物理起点。它与普通 volume 的 Attributes 可相同，
不能只用 Attributes 区分两种 offset 表形状。

图为数据组织关系，节点中的 80 为文件绝对字节偏移，跨度以字节表示：

#pele.diagram(r#"""
flowchart LR
    header[80-byte header: active offset words = 80] --> m0[Mip 0: W x H x D]
    m0 --> m1[Mip 1: clamped dimensions]
    m1 --> rest[Remaining mips: consecutive spans]
"""#)

该 profile 的完整像素区可按各级尺寸计算跨度并连续寻址；指定客户端的 volume
调度也按切片大小乘钳制深度推进目标，不读取该 offset 表设置每级目标。
完整连续负载与压缩组输出不足的 volume 是不同情况：头部形状或尺寸公式
不提供缺失字节的数值，不能把长度差额当作已知的零像素。

### Volume mip 与容器数据组

Win32 `2026.10.06.0000.0000` 的一组一 mip BGRA8 volume，mip `m` 的
完整字节数为 `4 × width_m × height_m × depth_m`；组表给出其编码数据的
物理位置，TEX header 描述其逻辑尺寸与 mip 身份。二者通过 mip ordinal
关联，物理块偏移的原点是 SqPack 条目体，逻辑 offset 的原点是 TEX 文件。

例如 `8×32×24`、六级 mip 的各级完整长度为
`[24576,3072,384,48,8,4]`，一类条目的组输出为
`[24576,1536,64,16,8,4]`。其中 mip 0、4、5 提供全部切片，mip 1、2、3
的字节数小于相应尺寸需求。读出 mip 4 的 8 字节表示原六级纹理的第 4 级，
其尺寸为 `1×2×1`；它不改变原头部的 mip 数、基础尺寸或相邻 mip 的长度。

#pele.diagram(r#"""
flowchart TB
    header[Original header: 8 x 32 x 24; 6 mips] --> dimensions[Mip 4: 1 x 2 x 1; 8 bytes]
    group[Type-4 group 4] --> payload[Complete 8-byte BGRA payload]
    dimensions --> image[Original mip 4: one 1 x 2 image]
    payload --> image
"""#)

图中的 4 为从零开始的 mip/组索引，尺寸为像素或切片数，不是 bit 偏移。
这类对应关系来自一组一 mip 的 volume 组织；数组或 cube 的组数还可包含
层/面维度，分组方式见 SqPack 格式。

## BC1、BC2 与 BC3 的块结构

### BC1

```text
偏移  长度  内容
0     2     RGB565 端点 c0
2     2     RGB565 端点 c1
4     4     16 个像素的 2 位颜色索引
```

RGB565 的低 5 位为 B，中间 6 位为 G，高 5 位为 R。颜色索引从最低位开始，
按块内逐行顺序对应像素。

当 `c0 > c1` 时，调色板包含两个端点和两个插值色：

```text
color[0] = c0
color[1] = c1
color[2] = (2 × c0 + c1) / 3
color[3] = (c0 + 2 × c1) / 3
```

当 `c0 <= c1` 时，`color[2]` 为两个端点的平均色，`color[3]` 为透明黑色。
颜色计算逐通道进行。

### BC2

前 8 字节存储 16 个像素的 alpha，每像素 4 位，从最低有效位开始按行排列。
后 8 字节为颜色块，其布局与 BC1 相同；颜色调色板始终使用四色模式。

### BC3

```text
偏移  长度  内容
0     1     alpha 端点 a0
1     1     alpha 端点 a1
2     6     16 个像素的 3 位 alpha 索引
8     8     颜色块（与 BC1 相同的布局，使用四色模式）
```

当 `a0 > a1` 时，两个端点之间生成六个插值值：

```text
alpha[0] = a0
alpha[1] = a1
alpha[k] = ((8 − k) × a0 + (k − 1) × a1) / 7    // k = 2..7
```

当 `a0 <= a1` 时，生成四个插值值，并保留全透明、全不透明两个值：

```text
alpha[k] = ((6 − k) × a0 + (k − 1) × a1) / 5    // k = 2..5
alpha[6] = 0
alpha[7] = 255
```

### BC4 与 BC5

BC4 是单通道格式，块长 8 字节，布局与 BC3 的 alpha 部分相同：两个端点各 1 字节，
随后是 16 个 3 位索引。BC5 的块长 16 字节，由两个 BC4 块组成，依次存储红色和
绿色通道。

Win64 TEX `0x6120` 与 `0x6230` 对应 BC4_UNORM / BC5_UNORM，端点为无符号
8 位数。下图原点为一个通道块的起点，位置单位为 bit；端点的数值比较与
打包 selector 的低位优先顺序分别定义，selector 整数按小端存储：

#pele.diagram(r#"""
packet
0-7: "Endpoint 0: u8"
8-15: "Endpoint 1: u8"
16-63: "16 selectors: 3 bits each, texel order"
"""#)

块内像素 `(x,y)` 使用 selector 整数的 bits `3×(4y+x)..3×(4y+x)+2`，
其中 `0 <= x,y < 4`。端点大小关系决定六插值或四插值加两个极值的调色板，
规则同上节。BC5 的两个通道各自比较端点、各自解码 selector，不共用模式
或索引。非 4 倍数的图像边缘仍有完整块存储，块内落在图像之外的 texel
不属于逻辑图像。BC5 存储 R/G；法线用途下的 Z 重建属于材质/shader 语义。

### BC7

BC7 的块长 16 字节。块内按位解释，最低位在前。第一个置位的位序决定模式：

```text
位       内容
0..n-1   模式位：n 个 0，随后一个 1
```

模式决定分区数、端点精度、P 位数量、索引精度与索引布局。模式 0、1、2、3、7 含
分区数据，每个像素按分区表属于某个子集；每个子集有自己的一对端点。模式 4、5
把颜色与 alpha 分开编码，并用 2 位旋转字段指出哪个颜色通道与 alpha 交换。模式
0、1、3、6、7 使用 P 位补充端点的最低有效位。

每个子集的第一个像素（fix-up 索引）省略索引的最高位，因为编码端保证该位为 0；
索引按块内逐行顺序连续读取。

## 参考资料

- [Lumina TEX 声明](https://github.com/NotAdam/Lumina/blob/8802d66a3cb3032be2fcf998bb10e864221bdb13/src/Lumina/Data/Files/TexFile.cs)：
  文件头、属性位及格式编号。
- [Microsoft：Block Compression](https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression)：
  BC 块布局和插值规则。
