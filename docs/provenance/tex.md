# TEX 多图像与浮点验证

短 volume 的完整单 mip 像素、公共 SqPack→TEX 输出与天空初始化采样路径
见 [Volume 单 mip 证据](volume-mips.md)。其中 273 个 base mip 和 672 个
完整单 mip 有逐字节结果，完整六级缺口与单 mip 验收分别记录。

## 来源与许可

基础格式声明与 BC 编码来源见既有 TEX 参考和源码 notices。多图像组织通过
Win32 安装 `2026.10.06.0000.0000` 的只读数据观察确定，未读取 legacy。
独立像素 oracle 为 Pillow 12.2.0（MIT-CMU，使用前通过 GitHub LICENSE 审查），
仅使用通用 DDS BC1/BC7、L8、BGRA 解码器，不是 FFXIV 解析库，不作为 Rust 依赖，
未复制其解码代码。Python struct 独立解释 RGBA4、binary16 与 binary32。

## 验证边界

`tests/fixtures/tex-observations.json` 固定十份样本的位置、文件 SHA-256 与语义行
SHA-256。包含两份 64 层 BC7 tile 数组、32 层 BC7 数组、RGBA4 cube、九级 BC7
cube、32 切片 L8 volume、RGBA16F LUT、两份 BC1 武器贴图和 BGRA8 图标。
`tools/verify_tex.py` 以 bounded raw DEFLATE 独立重建所有 type-4 字节，再对
所有 mip 的 1268 张图像逐像素比较，浮点 LUT 的 8192 个 f32 标量逐位比较，
其中 704 个值大于 1。Rust 的 Surface 与整 mip 输出也逐字节一致。

cube 图像计数字节实测取 0 或 6，两者均为六面。BC7 cube 和数组均为 mip 优先，
同级所有图像先于下级。volume 多 mip 的实文件证据见下文；其余浮点格式、
负数和非有限数具有受控回归，但不计为游戏文件证据；cube 空间面方向未确认。

type-4 的每组首块索引等于此前组块数之和，每组存储跨度等于对应 u16 跨度之和，
十份样本逐组核对。数据组不等同于 mip；volume 的 32 切片共用一组。

## 重现

使用 `examples/local_sqpack.rs` 按清单路径提取，或以 `examples/extract_entry.rs`
按 DAT 相对路径与绝对字节位置提取，输出置于仓库和安装目录之外。

```sh
nix develop -c cargo build --example validate_tex
nix shell --impure --expr 'let f = builtins.getFlake "github:NixOS/nixpkgs/a799d3e3886da994fa307f817a6bc705ae538eeb"; p = import f { system = builtins.currentSystem; }; in p.python3.withPackages (x: [ x.pillow ])' \
  -c python tools/verify_tex.py /path/to/game/sqpack /tmp/opencode/tex-check \
  /tmp/opencode/tile_norm_array.tex /tmp/opencode/tile_orb_array.tex \
  /tmp/opencode/float-lut.tex /tmp/opencode/volume-l8.tex /tmp/opencode/cube-rgba4.tex \
  /tmp/opencode/cube-bc7-mips.tex /tmp/opencode/bg-array-bc7.tex \
  /tmp/opencode/weapon_norm.tex /tmp/opencode/weapon_mask.tex /tmp/opencode/icon.tex
```

验证环境 Python 3.13.13，Pillow 12.2.0。原始字节、像素和运行报告不入库。

## 扩展库存与 volume 变体

`inventory_texture_headers.py` 按安装的 base/expansion index 做有界只读库存。
边界为 128 个 index、每张表 64 MiB、每 index 均匀选取至多 200000 个记录、
合计至多 2000000 个记录；每个类型/格式只保留六个候选位置。它不解压纹理，
不宣称样本可解码。运行环境 Python 3.14.8。

安装 `2026.10.06.0000.0000` 中发现 59 个 index，共 1802972 条记录；均匀选取
1576273 条，扫描到 360433 个 type-4 纹理头。`ffxiv/{020000,040000,060000,080000}`
四个 index 的记录数超过每表预算，其余表全部枚举；未使用 synonym 跳过分支。
有 482 个 volume 头，其中 453 个声明多个 mip。浮点候选有一份 `0x2460`
二维纹理和一份 `0x6330` 九级 cube；没有其他所查 float ID 的头命中，不能
据此推断全部资源不存在这些格式，也不覆盖 type-2 ATEX。

完整库存保留在外部 `texture-inventory-expanded-02.json`，SHA-256
`4ed440f13f6435805a9ea7711428d6bddc1b9b0dcd938f0dec621680d129a8f6`，
包括各 index 表摘要、选取数量、逐记录/头观察摘要和格式/mip 直方图。
首次前台运行到达 120 秒超时，没有完整报告；完成证据来自随后无超时的只读
运行，不把被中断的尝试当成覆盖。未启动服务器或游戏，也未修改安装。

### 全像素通过的两个 volume

`tests/fixtures/tex-variant-observations.json` 固定 index 路径/ordinal/hash、DAT
位置、原始头与重建文件和语义行摘要。独立 Python struct/bounded raw DEFLATE
重建全部 type-4 字节，再用 Pillow BGRX/BGRA 通用像素解码核对 Rust 输出：

- BGRA8 `8×32×2`，六级 mip，深度依次为 `2,1,1,1,1,1`，文件 2476 字节，
  合计七张图像/599 像素。宽、高、深度均独立逐级钳制到 1，而非以基础深度
  或最短边限制 mip 数量。
- BGRX8 `16×16×16`，一级 mip，文件 16464 字节，16 切片/4096 像素。
  RGB 来自 BGR 字节顺序，输出 alpha 为 255，不使用 X 字节作为 alpha。

`validate_tex` 同时核对按坐标、按索引与顺序遍历的图像访问，逐 surface 输出
与整 mip 解码相同；浮点格式还逐位核对两种输出路径。基础十份资源/1268
图像也用这一验证入口重跑，原始摘要和全部像素/浮点标量位保持一致。
受控回归使用实际六级矩形 volume 的尺寸轮廓，像素由测试自行构造，不入库游戏数据。

### 有真实候选的未解决分支

`tests/fixtures/tex-investigation-observations.json` 与
`observe_texture_group_sizes.py` 固定以下候选的头和每组真实解码字节摘要，
并重现 volume 的显式错误与 BC6H 零样本的解码结果：

- 两份 BGRA8 `8×32×24` / `8×32×32` 六级 volume，各组长度声明与对应
  raw/DEFLATE 实际输出一致；但前缀加所有组输出仅 26284 / 35052 字节，
  而资源声明总长为 28172 / 37540。第一级完整，后级存在差额。TEX mip
  跨度分别为 `[24576,3072,384,48,8,4]` 和 `[32768,4096,512,64,16,4]`；
  压缩组实际解码为 `[24576,1536,64,16,8,4]` 和
  `[32768,2048,128,16,8,4]`。差额字节内容未独立建立，完整文件读取返回
  `decoded size disagrees with entry size`，没有猜测补零或错移后级数据。
- ex5 一份 `0x6330` BC6H cube，`256×256`、六面、九级 mip，524528 字节。
  type-4 重建完整通过，54 组是每 mip 的六个面，32778 个压缩块全零。
  [BC6H 专项](bc6h.md)确认指定客户端的 SF16 映射，54 个 mip/面浮点图像
  与零值 oracle 逐位一致；非零 signed/HDR 使用独立官方 mode 11 受控实验。
  该实文件不能代表非零 HDR 或所有模式，不能由 RGBA16F/BC7 验证替代。

前缀与各组直接拼接的规则适用于具有完整长度证据的条目；尺寸公式不确定
缺口内容。完整单 mip 的读取及 272 条天空消费关联见上述专项证据；缺失
低 mip 和非零实际 BC6H 各有独立的样本/数值验证边界。

```sh
python tools/inventory_texture_headers.py /path/to/game/sqpack /tmp/opencode/texture-inventory.json
nix develop -c cargo build --features local --example extract_entry --example validate_tex
# 使用本页列出的带 Pillow 的 Python 环境：
python tools/verify_texture_variants.py /path/to/game/sqpack /tmp/opencode/tex-variant-check
python tools/observe_texture_group_sizes.py /path/to/game/sqpack /tmp/opencode/tex-group-check
```

### 压缩块调度与 volume 目标跨度

固定 executable（身份同 [BC6H](bc6h.md)）的普通离线 objdump 搜索在首选 VA
`141d0cb30..141d0cbea` 定位到一个 raw/DEFLATE 单块助手，186 字节，区段 SHA-256
`a330df8dbb1f6a0c253ad2b437a478f05394ba0f0e464d2c7c84f59713b5bea8`。
它从 source 对象 `+8` 取得块指针，读取 16 字节头，对 `+8` 的 compressed
size 与 32000 比较，`+12` 为声明输出长度。raw 分支按该长度经栈buffer复制；
压缩分支调用解码helper后，按长度槽复制到任务对象 `+0x78` 提供的地址。
长度槽初值16384，wrapper只在decoder返回1时更新它，外层不检查返回值；
字段映射与错误路径限定见[块任务证据](volume-block-loading.md)。

固定程序的间接调度链如下；地址为首选 VA，均为离线指令观察，不是运行观测
或 REA Evidence。REA 原生引擎限制见来源总表，没有因此执行目标或修改环境。

- `1405e6df0..1405e6efd` 按参数取得描述符表 `1428ac090` 的一个指针，
  复制描述符并设置任务参数。第 6 项 `1428ac0c0` 指向 `1428cb8c0`，该
  描述符的首指针是单块助手 `141d0cb30`。所以直接 call 搜索不能替代这条链。
- `140479600..14047964a` 是该分派函数的包装入口，转交编号、源块、长度和
  目标等参数。`140304510..1403045d0` 的块循环用编号 6 调用该入口：源地址
  按 u16 stored span 推进，目标地址按块头 `+12` 的 expanded size 推进。
  任务描述符中 `+0x78` 的地址正是传入目标，不根据 compressed size 推进。
- `140303140..1403031c4` 从资源头 `+0x14` 取得记录数，以 `header+24`
  为 20 字节记录起点，记录表后取得 u16 span 表。该结构与真实 type-4
  header/记录/span 数据独立相符。
- `1403035e7` 的 `0x01000000` 纹理类型分支进入
  `1403038ee..14030399c`。每级从 20 字节记录的 `+12/+16` 取得块起点与
  数量，经 `140304510` 提交；下一组源起点按记录 `+4` 的存储跨度推进。
  mip 目标按切片字节数乘 `max(1, depth >> relative_mip)` 推进，不按该组
  实际解压长度推进。切片大小助手 `140219d30..140219dd8` 对 width/height
  逐级钳制，并区分普通与块压缩行大小。这里没有读取 TEX 的 `+28` mip 表
  来设置目标；文件 mip 表与上传缓冲区的跨度需分别表达。

这些区间没有提供两份异常 volume 的缺失数据来源。下述构造器安装的分配器
路径没有保证 payload 初始化；上传时如何处理未被任务覆盖的区间、这两份
资源是否走到所述路径仍未直接证明。
不能从静态目标推进规则得出空余字节等于零，也不能把显存上传布局当成完整 TEX
重建算法。没有移植或模拟这些函数，库仍拒绝把不完整组拼成成功文件。

`verify_client_texture_groups.py` 固定 executable 身份、十五段区间摘要、十二个锚点、
七个直接 call 目标、三个分配虚表槽及两级描述符指针。它只复现静态证据，不计为 volume 解码
验收。`observe_texture_group_sizes.py` 另以真实头的 mip slot 和实际组长度列出
候选放置后的未覆盖尾区间，差额合计分别为 1888 / 2488；只列地址，不创建
补零文件或宣称读取了运行中缓冲区。

```sh
objdump -d -Mintel --start-address=0x141d0cb30 --stop-address=0x141d0cbea \
  /path/to/game/ffxiv_dx11.exe
python tools/verify_client_texture_groups.py /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/client-texture-group-check
```

### Volume 上传缓冲区的分配路径

身份、平台和工具沿用上节的固定 Win64 executable 与 GNU objdump 2.46；
以下地址均为首选 VA。观察对象是离线指令，不是内存快照或运行中的堆。
没有移植、执行或模拟分配器，没有读取第三方 FFXIV 实现。

- `1403034a4..140303580` 将每 mip 切片大小乘钳制后的 depth 求和；
  `140303551..14030356d` 从全局 `1428fd958` 的管理器取 `+0x10` 指针，
  以虚表 `+0x10` 分配目标缓冲区，`r8=16` 为传入对齐值。
  返回地址保存到资源对象 `+0x148` 并作为后面的写入目标。
- `1400cfb9c..1400cfbe4` 创建管理器并绑定该全局，调用
  `140338260..140338515` 初始化堆数组。循环将分配器对象安装到管理器
  `+8+i×8`；故 `+0x10` 对应此数组的第二项。
- 构造器安装三个虚表：对象首部 `142154868`、子对象 `+8` 的
  `142154818`、子对象 `+0x90` 的 `1421547c8`。它们的 `+0x10`
  槽分别指向 `1403377f0`、`140336ae0`、`1403358c0`。
  `1403377f0..14033780b` 增加计数后转交 `+0x90` 子对象；
  `1403358c0..140335909` 在普通u32容量与16-byte对齐下，将大于128字节
  的请求转交池的 `+0x108` 指针；实际比较使用大小类索引的低32位。
  该指针在构造器中设为外层对象的 `+8`，抵达大块分配函数。
- `140336ae0..140336cab` 可以在已有区域查找空间，经
  `140336b5a` 调用 `140336420..1403366fc`；新区域分支也调用这个
  子分配函数。子分配函数没有函数调用；它选择空闲区、对齐返回的 `rbx`，
  修改链表/计数及返回地址之前的64字节分配头。有效区域、链表、元数据
  不别名payload及正常ABI下，这些本地写入不覆盖返回payload。
  成功后的排序helper及新区域分支见[分配器写入范围](volume-block-loading.md#分配器与元数据写入)。

据此，构造器安装的已有区域路径本身不提供“每次分配都返回全零字节”的
保证；不能将新提交页可能为零的性质推广为所有堆分配的性质。这个观察不
证明两份资源实际执行了该路径，也不证明缺口在运行时非零。管理器替换、
其他路径及上传端消费仍须分别追踪。两份文件的 1888 / 2488 字节差额不能
据此变成可复现的 TEX 字节，完整提取保留显式错误。

`tests/sqpack_resource.rs` 的六组 volume 回归使用自行生成的 payload、上述
独立观察的 `8×32×24/32` 尺寸、六级 mip offsets 和各组输出长度。每组的
块输出与组声明均匹配时，两种缺口仍返回长度不一致；对照产物为各 mip
提供足量自行生成的字节，完整提取及 TEX 全 mip 借用通过。对照产物不补全
实际资源、不作为实际缺口内容的证据。静态区间、虚表槽及锚点保存在
`tests/fixtures/client-texture-group-observations.json`。其旧临时报告目录缺失；
`volume-allocation-13`的固定范围重捕获与6个旧allocator摘要相符，独立身份
由`tests/fixtures/volume-allocation-observations.json`绑定，不恢复旧报告身份。

## R16F 与 R11G11B10F 的映射和数值证据

同一固定 executable 的 `140218100..140218389` 格式映射函数中，
`1402181b4..1402181c7` 的连续差值比较确定 `0x1450+1+0xcef=0x2140`，
命中 `1402181dc` 返回 54；`140218202` 比较 `0x2350`，命中
`140218220` 返回 26。GNU objdump 离线观察，不执行或模拟客户端。函数摘要
复用 `bc6h-observations.json`，独立锚点在 `verify_float_texture_controls.py`
校验。此前已审查 MIT 的 DirectX-Headers commit
`faf928dc6feb8b86e9e3d6a39fc5171de7d8ac36`，再次读取 `dxgiformat.h` 枚举：
54 是 R16_FLOAT，26 是 R11G11B10_FLOAT。这是 ID 与像素编码的独立映射证据，
不依赖资源的文件名或只有零的纹理。

Rust 的 R16F 复用借用 mip 与 binary16 数值转换；R11G11B10F 分别提取
三个数值位域，把五指数位及六/五尾数位零扩展到 binary16，再复用数值转换。
它不把打包字当成三个连续 f32，不借用 native 的分支结构。RGBA32F 的 HDR/
非有限类别保留，RGBA8 使用同一公开 clamp/round 规则，调用者的输出预算覆盖
全部图像。NaN 的 payload/sign 不是 half/packed 数值接口保证的一部分。

独立 CPU oracle 使用已审查的 Microsoft DirectXMath 固定版本，来源、许可证
和 Inc 清单摘要见 [BC6H 通用 oracle](bc6h.md#microsoft-cpu-多模式-oracle)。
额外阅读该固定版本的 `DirectXPackedVector.h` 的 XMFLOAT3PK 声明及
`DirectXPackedVector.inl` 的 XMLoadFloat3PK 实现；构建使用原始头，不把实现
移入库。`tools/bc6h_oracle/float.cpp` 只是调用 XMConvertHalfToFloat /
XMLoadFloat3PK 并输出 RGBA 的自行编写适配器，没有 TEX/SqPack 解析。
`build_bc6h_oracle.py` 校验相同来源摘要后生成 `float-oracle`，保留许可证和
编译器/输入/可执行文件摘要；没有 Cargo 或 WASM 的 C++ 依赖。

受控语料包含全部 65536 个 binary16 位模式，以及遍历所有 2048 个 R/G、
1024 个 B 子通道编码的 packed 字和 4096 个 SHA-256 确定性 packed 字。
两种格式分别放入 2D、三层 array、六面 cube、三级 shrinking-volume，另三种
布局各含三级 mip。`verify_float_texture_controls.py` 通过公共 TEX API 验证
全部图像；291456 个 f32 分量中，284613 个非 NaN 的有限值、带符号零或
无穷逐位相等，6843 个 NaN 的位置/类别一致。NaN 在生成语义摘要时统一为
`0x7fc00000`，不是通过容差接受不同有限值；原始两侧结果仍在外部目录保存。
所有 RGBA8 像素也由独立数值输出量化后比较，单位 alpha 不来自压缩负载。
同一语料还与 Python struct 的 binary16 转换、按指数/尾数公式用 ldexp 求值
的 packed 转换分别比较；普通联跑仍复算这一路数值 oracle，并核对已固定的
Microsoft 输出摘要，不需要编译器或外部可执行文件。

`tests/fixtures/float-texture-control-observations.json` 固定八份受控 TEX 及
数值/像素摘要。回归另覆盖正负零、最小 subnormal、负值、最大有限值、Inf/
NaN、三个 packed 通道的位宽和次序、截断负载和输出预算。真实资源库存的
float 筛选包括两个 ID；没有因此声称取得它们的实际 TEX/ATEX 文件，实样本
缺口与受控数值证明分别保留。

包含这两个 ID 的 type-4 筛选复跑检查 59 个 index、1576273 条选中记录和
360433 个 type-4 头，没有 `0x2140/0x2350` 命中，报告 SHA-256 与上述扩展
库存固定记录 `4ed440f13f6435805a9ea7711428d6bddc1b9b0dcd938f0dec621680d129a8f6`
一致。VFX type-2 的 100000 条首块调查也复跑并匹配原固定摘要，没有这两个
候选。两次都是有界负向观察，不把缺失样本解释为格式 ID 不存在。

```sh
# 按 BC6H 来源页的固定源码构建命令生成 float-oracle：
python tools/verify_float_texture_controls.py /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/float-control-check /tmp/opencode/bc6h-oracle-build/float-oracle
# 只复跑已固定的独立数值摘要：
python tools/verify_float_texture_controls.py /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/float-control-digest-check
```

## BC4 / BC5 的实际纹理与端点语料

来源与许可沿用本页已审查的 Pillow 12.2.0（MIT-CMU）通用 DDS CPU decoder，
以及 DirectX-Headers commit `faf928dc6feb8b86e9e3d6a39fc5171de7d8ac36`
（MIT）。再次读取该固定版本的 `include/directx/dxgiformat.h`，确认
`DXGI_FORMAT_BC4_UNORM = 80`、`DXGI_FORMAT_BC5_UNORM = 83`。只使用通用
像素 oracle 与枚举事实，不读取新增 FFXIV 解析实现、不引入游戏格式库或
移植 Pillow 代码，亦无 GPU 或游戏进程参与。

指定 executable 的既有格式映射区间 `140218100..140218389`，SHA-256
`096a14280cd43a1bac8362118f4869d2c6f2846978fc63b24aae559a347b1fa5`，另有
以下独立静态映射：`14021823e` 比较 `0x6120`，相等跳到 `1402182fe`
返回 80；`140218312` 比较 `0x6230`，相等跳到 `140218350` 返回 83。
`verify_bc6h.py` 在复验同一固定函数和 executable 身份时也检查这些比较/
分支/返回值锚点。这些是离线 GNU objdump 2.46 指令观察，不是运行或 REA
Evidence；不能单从 TEX 文件后缀或库枚举名称推出 UNORM/SNORM 的选择。

### 实际资源

`find_bc45_textures.py` 在每索引最多 100000 条、总量 500000 条的界限下
均匀选择记录，先检查角色主索引。在检查 77236 条选中记录后取得两个
BC4 和两个 BC5 候选即停止。它只做 type-4/TEX 头部筛选，不把候选称为
解码通过。安装来源为只读 Win32 `2026.10.06.0000.0000`。

`bc45-texture-observations.json` 固定四个 index ordinal/hash、DAT 位置、
头部/完整资源摘要及完整语义行摘要：

- BC5：`512×512` 和 `256×512`，各十级 mip；
- BC4：两份 `256×256`，各九级 mip。两份的头部相同，完整字节摘要不同。

`verify_texture_variants.py` 核对每个 index 位置，使用 Python struct 与有界
raw DEFLATE 独立重建 type-4 全部字节，再与 Rust 提取相等。每张图像包装
标准 DX10 DDS header，用上述 DXGI IDs 交给 Pillow，未将 Rust 像素回送成
oracle 输入。全部 38 张 mip 图像、699050 个 RGBA8 像素逐字节相同；Rust
按坐标/索引/顺序访问以及逐图像/整 mip 解码也相同。包含 `2×2`、`1×1`
及非正方形末级的块边裁切。

报告按通道统计实际编码块的两种端点关系、相等端点和八种 selector 的次数。
四份实文件的每个通道都命中 `a>b`、`a<=b` 与所有 selector；统计包含块边
落在逻辑图像之外的编码 texel，不冒充额外逻辑像素。完整报告摘要固定为
`5f26337bea88be128519bf2dcfa7aa699e7a6923a09da229bfd2bd0331d61ce5`。

### 全端点对受控验证

`verify_bc45_controls.py` 自行生成两个 `1024×1024` TEX，分别为 BC4 和 BC5。
每通道遍历全部 65536 个有序 u8 端点对，每个 4×4 块把八种 selector 各使用
两次；BC5 的 G 通道交换端点并反转 selector，检验通道与分支独立性。
每通道有 32640 个 `a>b`、32896 个 `a<=b` 块，其中相等端点 256 个；
每种 selector 各出现 131072 次。两张受控图像的 2097152 个 RGBA8 像素
均与同一独立 Pillow DDS oracle 逐字节一致，不以容差放宽插值差异。

原有回归之外，Rust 的小型固定向量同时覆盖 R 的四插值/极值模式和 G 的
六插值模式，以反向 selector 检查独立红绿和 alpha=255、B=0 的输出契约。
`bc45-control-observations.json` 固定生成语料与语义报告摘要；报告 SHA-256 为
`17486a260f7b3f495b7988022da5cf04295b05d8510c40eda4a524ef5c92165b`。
端点对穷尽不等于所有 selector 排列和全部纹理变体穷尽；这些生成物不是游戏
生成文件。源资源、生成纹理、像素和完整输出均留在仓库外。

```sh
# 使用本页固定的 Pillow 环境，并构建 extract_entry / validate_tex：
python tools/find_bc45_textures.py /path/to/game/sqpack /tmp/opencode/bc45-discovery.json
python tools/verify_texture_variants.py /path/to/game/sqpack /tmp/opencode/bc45-real \
  tests/fixtures/bc45-texture-observations.json
python tools/verify_bc45_controls.py /tmp/opencode/bc45-controls
```

## Volume 库存、重复 offset 与完整像素

`verify_volume_inventory.py` 使用与纹理头库存相同的选择界限：至多 128 个
index、每表 64 MiB、每表均匀选取 200000 条、总计 2000000 条记录。安装
仍为 Win32 `2026.10.06.0000.0000`。实际在 59 个 index 中选择 1576273 条
记录，发现 482 个 volume 头，没有重复 DAT/offset 位置。每个 volume 最多
16 MiB、累计组输出 128 MiB、累计逻辑像素 64000000 的额外界限均有检查。
没有读取第三方解析实现，独立像素 oracle 沿用已审查的 Pillow 12.2.0。

每条记录的所有 raw/DEFLATE 块均独立解压，检查 stored span、首块索引、
块输出与组声明，再与尺寸/深度/级数计算的 mip 跨度比较。完整输出必须与
Rust type-4 提取字节逐字节一致；不足者必须重现明确错误且不生成 TEX 文件。
209 条完整记录的 1940 张图像、2370564 个像素与独立 Pillow/struct 全部相等，
其余 273 条不能计为完整重建通过。

### 全部有效 offset 等于 80 的头部

其中五个实际头部的有效 SurfaceOffsets 均为 80：四条具有完整组输出，
一条有缺口。这些头与普通 volume 的 Attributes 可以相同。完整例子为
`ffxiv/010000.win32.dat1@967331456`，BGRA8 `8×32×2`，六级 mip，2476
字节，完整资源 SHA-256 为
`9d8313c8a34eb2ec5a78c15173f9ad36793164416679b26ccb3f7b81ba39e2b6`，
头部 SHA-256 为 `c496af78f796feca81c53ba1d118061d092ca3fcafe946507fc3b89dbd9a89f1`。
组长度 `[2048,256,64,16,8,4]` 精确填满计算的六级 mip。源 offset 未被
重写，顺序消费这些实际组字节的所有像素都与独立 oracle 相等。

该数据组织还与前述固定客户端 volume 目标推进的静态观察相符：它从资源
对象尺寸计算每级 span，不用 TEX offset 表确定 mip 目标。此对应关系不
证明这五个具体条目在一次运行中采用了哪个分支，也不引入客户端的分配/
上传实现。对这种头部建立显式的 packed-volume 借用解析 profile，其依据
是实际完整字节和尺寸/组边界对照，不是只由错误 offset 推测数据存在。

`Tex::parse_packed_volume` 只接受 volume 且全部有效 offset 为 80 的头部，
严格核对全部连续 mip payload；其他 offset 形状、不一致尺寸、非 volume
与截断数据均拒绝。普通 `Tex::parse` 仍按声明 offset 校验，避免把重复偏移
泛化成任意输入的容错开关。`declared_mip_offsets()` 保留有效原始值，
`uses_packed_volume_layout()` 显式标识读取方式。回归覆盖全部前缀截断、
未知重复形状、错误类型和六级负载/预算，不合成任何缺失像素。

### 有缺口的长度族

两份固定见证所在的长度族在这组选择中共 273 条：126 条 depth 24、147 条
depth 32，均为 BGRA8 `8×32`、六级 mip。每条组长度都等于
`slice_bytes(m) × max(1, depth >> (2×m))`，而 TEX mip 目标跨度对应
`slice_bytes(m) × max(1, depth >> m)`；差额分别为 1888、2488 字节。
其中 272 条使用普通递增 offset，一条也具有重复 80 的 offset 形状。因而
重复 offset 与组输出不足是两个独立问题，packed profile 没有解除这些差额。

这条精确数值轮廓是长度观察，不证明缺失字节值、作者工具算法、资源用途或
可行的重排/缩放规则。库没有将 double-depth 轮廓当成解码器，没有补零、
复制 slice 或重采样 base mip；273 条的完整文件读取重现长度不一致错误。
其中完整组的单 mip 像素对照与原始头保留见 [Volume 单 mip 证据](volume-mips.md)。

`tests/fixtures/volume-inventory-observations.json` 固定完整报告摘要与计数，
报告逐项保留位置、头部/组摘要、实际和理论跨度、成功像素摘要或明确缺口。
完整报告 SHA-256 为
`d0c538c50cc4de5cc6978ef25ecac2b0a3686c2d2a6518f3249317be504b3e69`。
原始资源、所有像素与逐字段输出留在外部目录。联跑将它列为 inventory
observation，明确区分完整像素验证与缺口复现，不把任务返回成功等同于
482 条全部重建成功，也不宣称遍历整个安装。

```sh
nix develop -c cargo build --features local --example extract_entry --example validate_tex
# 使用本页固定的 Pillow 环境：
python tools/verify_volume_inventory.py /path/to/game/sqpack /tmp/opencode/volume-inventory
# 只对已提取的重复-80 profile 读取全部 mip：
target/debug/examples/validate_tex /tmp/opencode/packed-volume-pixels --packed-volume /path/to/volume.tex
```

## Volume 对照条目与初始化描述

### 完整 counterpart 的有界比较

`observe_volume_counterparts.py` 先验证上述 volume 库存的固定报告摘要，
再按 `(width,height,depth,format,base-mip SHA-256)` 比较完整与有缺口条目。
273 个缺口分属 177 个 base-mip 组；209 个完整条目中没有匹配的 counterpart。
因此这组输入没有可用于检验低级 mip 内容的同 base 对照。即使 base 摘要
相等，也不能直接据此认定低级 mip 相同。

该工具另独立读取安装内 59 个 `.win32.index` 的完整主 hash 表，共 28847552
字节、1802972 条记录，每表记录数不作抽样。每表记录 packed 字段低位和
表内重复 hash 数均为零；各表摘要和记录数保留在报告中。界限为 128 张表、
单表 64 MiB、累计 128 MiB。这个观察不读取 synonym 名称表或 index2，
不解析条目名，也不把索引覆盖等同于 DAT 内容的完整检查。它不能证明未被
volume 库存选中的资源没有其他对照。

固定安装/平台沿用上节。`volume-counterpart-observations.json` 固定报告
SHA-256 `67078b1c1f5af93006dee5a8565dd5a1cf337a344ff1a767e5f542feb32cf764`。
它属于负向库存证据，不计为成功解码或缺失字节恢复。

### staging 到纹理创建参数的静态链

固定 executable、GNU objdump 及 REA 环境边界沿用本页；这不是运行观察，
没有执行客户端、模拟上传或调用图形接口。离线阅读范围包含
`140303480..140303660`、`1403038ee..140303b90`、
`14030418b..140304370`、`140304780..140304b50`、
`140216dc0..140217120` 和 `140218390..140219120`。关键范围及锚点加入
`client-texture-group-observations.json`，由原静态复验工具检查：

- `14030484f` 从资源 handle `+0x148` 取得 staging，`+0x130` 取得 texture，
  经 `14030485d → 140216dc0 → 140218390` 进入初始化函数。
  `140304af6` 是同一 helper 的另一条件调用点；这些调用点随后释放 staging。
- 初始化函数将输入指针保存在 `r12`，再以 `r15` 逐 mip 推进。
  `140218444..140218455` 清零的是栈上 `rbp-0x10` 开始的 0x8000 字节描述区，
  不是输入指针指向的像素区。`+0x1000` 条件分支还可分配复制缓冲区，不能
  将描述表初始化与这些缓冲区或 staging 的内容混为一谈。
- BGRA8 不走 `0x1130/0x1440/0x1441` 格式转换分支。
  `140218711..14021879d` 将源指针、row pitch 和 slice pitch 写入每个
  16 字节描述；source 按 `slice_size(m) × max(1,depth >> m)` 推进。
  这里没有按 SqPack 实际短组长度缩小所需深度，也没有由 TEX 的 offset 表
  找到省略区内容。
- 类型掩码为 `0x01000000` 时进入 `140218fdf`。栈上创建描述依次接收
  width、height、depth、mip count；初始数据参数指向上述逐 mip 描述。
  仅在输入指针为空时改传空初始数据。`140219051` 通过接口槽 `+0x30`
  间接调用。布局和分支与 3D 纹理创建相符；本项只固定调用点及参数数据流，
  不把间接接口的运行目标或返回值冒称为已执行验证。

该链要求的 mip 跨度与此前独立尺寸计算一致，没有提供省略区像素的确定性
初始化。它不能证明 273 条具体资源都选取此路径，也不排除未追踪的调用前
状态或其他写入。缺口字节仍无可验收的恢复规则，库继续明确拒绝短输出，
不移植客户端的分配、缓存或接口调用。

静态复验覆盖 19 段、24 个锚点和 12 个直接调用目标，并固定既有描述符及
分配器虚表位置。原始程序字节、完整反汇编和报告留在外部目录。

```sh
python tools/observe_volume_counterparts.py /path/to/game/sqpack \
  /tmp/opencode/volume-inventory/report.json /tmp/opencode/volume-counterparts
python tools/verify_client_texture_groups.py /path/to/ffxiv_dx11.exe \
  /tmp/opencode/texture-initialization-static
```

## Cube 面序与类型化视图

### 公开 API 资料与许可

资料作者/发布者为 Microsoft。先审查 MicrosoftDocs/win32 的根 LICENSE，
再核对 MicrosoftDocs/sdk-api 的根 LICENSE 字节与其完全相同：
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)，18650 字节，
SHA-256 `7183ef9749eb9b75e710a81a4c812a9405cc184f087500581a973e5929ac3f5e`。
阅读的是 API/概念文档，不是第三方 FFXIV 实现；没有引入依赖或转载其插图、
示例实现。以下为本项目对数据契约的摘要与本地独立观察，不是逐句翻译。

固定阅读来源：

- [MicrosoftDocs/win32](https://github.com/MicrosoftDocs/win32/tree/51c9b7cc54ef393b6406c8320a7840fdd88209d5)，
  commit `51c9b7cc54ef393b6406c8320a7840fdd88209d5`，
  `desktop-src/direct3d11/overviews-direct3d-11-resources-textures-intro.md`：
  SHA-256 `9222933349acf32e93fb485b2cd6a2c418fe10d066e5ec8e6417b35fcf67cbc5`；
  同目录 `overviews-direct3d-11-resources-subresources.md`：
  `9f687d238e97bc7ea7f5f830840ab53a27b76ade0a647ca69eb8ffe3a229195e`。
- [MicrosoftDocs/sdk-api](https://github.com/MicrosoftDocs/sdk-api/tree/656e9656bfaef34dd29cd7ee9e1146aec17e924c)，
  commit `656e9656bfaef34dd29cd7ee9e1146aec17e924c`，
  `sdk-api-src/content/d3d11/ne-d3d11-d3d11_texturecube_face.md`：
  `7338b5ca95f931daa911a63d29c8202d50a0f04b7a95356838404f625ed90b06`；
  同目录 `nf-d3d11-d3d11calcsubresource.md`：
  `1636edfbdee1d012c605b816a46f53ed8a28893cabbacfbdcfa54244a71ff0d3`。

资料给出 cube 使用六个 2D array slice，面枚举 0–5 对应
`+X,-X,+Y,-Y,+Z,-Z`，subresource 索引为 `mip + slice × mip_count`。
这些 API 事实本身不证明任意 TEX 的 ordinal 如何传给接口；关联来自下面
独立检查的客户端描述表数据流与实际文件图像组织。

### 本地静态关联

executable、GNU objdump、REA 环境与授权沿用本页；没有运行图形接口。
阅读 `1402187b0..140218a4e` 并复用既有
`140218ec2..140219060` 的创建分支：

- 按 mip 外循环、face 内循环推进输入，每个面递增一个完整图像跨度；
  `140218a09..140218a1f` 的直接字节分支把源指针写入描述下标
  `face × mip_count + mip`。它只改变描述表编号，不交换 face ordinal。
  packed/L8 的其他分支调用格式转换助手；本项不由那些助手推断面内旋转。
- cube 类型的创建分支在 `140218f69` 写入 array size 6，`140218f94`
  写入 cube misc 值 4，并以这张描述表经接口 `+0x28` 创建资源。与上述
  Direct3D cube/subresource 契约相接后，可将文件 face ordinal 命名为
  Direct3D 六面枚举。它不是一次运行中间接接口目标、资源路径选择或采样
  结果的观察，不为环境图定义世界轴。

两段与五个锚点加入 `client-texture-group-observations.json`；静态复验共
21 段、29 个锚点。这个编号关联不解决 volume 缺口，也不推广未知的 cube
array 头部或采样向量到 UV 的公式。

### 真实图像字节与独立验证

沿用 `tex-observations.json` 的两个固定安装样本：
`ffxiv/000000.win32.dat0@23470336` 的 RGBA4 cube，ArraySize 为 0、一级 mip；
`ffxiv/020000.win32.dat0@4945118080` 的 BC7 cube，ArraySize 为 6、九级 mip。
文件 SHA-256 分别为
`ef20bb4626254380d07d6c1eefea354d61041b3f3499a162b6d7172db88974a4`、
`17832f9b45fd036327ffbca08b3080970debb8fd46b4ad97fb43cff4a2c26d5c`。
两者的 base mip 均有六个互不相同的面摘要，不能用全相同面假装证明 ordinal。

`CubeFace` 明确提供这套面名和数值，`Mip::cube_face` 返回未经旋转/翻转的
借用图像；2D、六层 2D-array 和六切片 volume 均明确报 NotCubeTexture。
该 API 仍不解析其他 cube array-size 编码。回归覆盖两种 cube 头、六个
面与多个 mip、非法 index 和上述非 cube 类型，保留原始数字索引 API。

`verify_cube_faces.py` 在核对原始资源摘要后，用 Python struct 独立取得
每级 offset，按 RGBA4 像素宽度或 BC7 压缩块大小计算每张二维图像跨度，
逐字节比较 Rust 类型化 face 导出的全部 60 张 encoded 图像；另对面名、
ordinal 与尺寸行逐项比较。此项验证名称视图不重排文件字节，不根据图片
内容猜测朝向；既有独立像素解码回归仍执行。

`cube-face-observations.json` 固定来源文档摘要、面名和报告 SHA-256
`4be072887e13152966fc703f94b0956600d8448250e871147381e8b6a2b6ffe2`。
原始图像、API 文档副本、完整反汇编及导出 face 字节均留仓库外。
没有启动游戏、GPU 或服务器。

```sh
nix develop -c cargo build --features local --examples
python tools/verify_cube_faces.py /path/to/cube-rgba4.tex /path/to/cube-bc7-mips.tex \
  /tmp/opencode/cube-face-bytes
```

## Volume 完整主索引覆盖与 index2 对照

`verify_volume_inventory.py --all-records` 保留总计 2000000 条记录的上限，
但逐表不使用 200000 条的选择上限，并断言每张发现的主 index 表均被完整
访问。固定安装中共有 59 张表、1802972 条记录；与上节选择相比，四张
大表多检查 226699 条。所有 volume 头、组长度、样本身份及像素输出与
既有 482 条记录相同：209 完整、273 缺口、1940 图像、2370564 像素。

完整报告 SHA-256 为
`a4058e67025c06a8c4d432a7fc99e9ea1455288b78578bfed84b40a27836226b`。
`volume-full-inventory-observations.json` 固定该身份；报告中的每表 selected
等于 records。以该完整报告进行 counterpart 比较，273 个缺口仍属于
177 个 base-mip 组，没有同尺寸/格式/base 摘要的完整对照。对应 counterpart
报告摘要为 `1c9415d01699cdd79a8ea765d9512f62fbb94c3b47ce522e819205069ff9058c`。

[index2 对照](sqpack-index2.md)另检查所有 59 对索引，普通位置和 62 个
碰撞名称记录展开后的 packed 多重集与主索引相等，没有只在 index2
出现的 DAT 位置。这里只扩大已索引记录的覆盖，不把它说成整个 DAT 空间、
orphan 记录、未知包或所有历史安装的穷尽。完整块输出不足与客户端分配/
初始化链仍不能确定缺失像素，库继续明确拒绝短输出。

```sh
python tools/verify_volume_inventory.py /path/to/game/sqpack \
  /tmp/opencode/volume-full-inventory --all-records
python tools/observe_volume_counterparts.py /path/to/game/sqpack \
  /tmp/opencode/volume-full-inventory/report.json /tmp/opencode/volume-full-counterparts
```

## 全主索引 type-4 纹理头覆盖

`inventory_texture_headers.py --all-records` 对固定 Win32 安装
`2026.10.06.0000.0000` 的全部 59 张主 index 表选择全部 1802972 条记录，
读取各 DAT 条目公共字段及 type-4 的 80 字节 TEX 头。总计 405685 个
type-4 头；相对于逐表选择上限 200000 的库存，多检查 45252 个 type-4
头。该模式保留 2000000 条总预算、128 张表和单表 64 MiB 的边界，并断言
每张发现表的 selected 等于 records。

观察到的格式 ID 为 `1130/1131/1440/1441/1450/1451/2460/3420/3430/3431/
6120/6230/6330/6432`。已登记浮点 ID 中仍只有一个 `0x2460` 2D 条目；
`0x6330` 仍只有 `ex5/020505.win32.dat0@94043776` 的零 cube。二者的
位置、头摘要与既有样本一致。没有据此取得非零实际 BC6H 或其他 float ID
样本；完整头检查不等于全部像素验证。

`tests/fixtures/texture-full-inventory-observations.json` 固定报告 SHA-256
`a738aa41634cebfe4cc30f52e5d583942da80b22c2a4326918170b7602b97969`。
报告包含每张主表与所读字段摘要、各 type/format/mip 的计数及候选身份。
覆盖边界为已发现主表中的 type-4；type-2 负载、未索引 DAT 数据、未安装包
和其他版本均不属于该负向结果。

```sh
python tools/inventory_texture_headers.py /path/to/game/sqpack \
  /tmp/opencode/texture-full-inventory.json --all-records
```

## 全主索引 type-2 TEX 头候选覆盖

`inventory_standard_textures.py --all-indexes` 对上述固定安装的全部 59 张
主表选择全部 1802972 条记录；跨分类搜索，不将 VFX 包视作 ATEX 的唯一
来源。实际可定位的反例包括 index2 名称表中的
`bgcommon/world/common/vfx_for_bg/texture/bomb101zy.atex` 与
`chara/monster/m0907/obj/body/b0004/vfx/texture/kira_f02m_n1.atex`，它们
是 VFX 分类之外的 type-2 ATEX。名称、位置及整条目提取字节的证据归于
[index2 碰撞对照](sqpack-index2.md)，不据文件所在分类推断纹理像素格式。

此模式总预算为 2000000 条记录、32 GiB 首块解压数据、最多 128 张表，
单表上限 64 MiB。只解压 kind=2、声明目标在 80 字节到 64 MiB 之间的
资源首块；检查压缩块跨度、DEFLATE EOF 与输出长度。TEX 候选要求首块
至少有完整 80 字节头、已知 type/format、合理尺寸和严格递增的声明 mip
偏移。一个候选头不证明后续全部块、mip 或像素有效；跨首块分割的头和
超出预算的资源也不能由这条路径排除。

实际检查 1184882 个合乎条件的首块，解压 9665818722 字节；227828 个
候选头的 type/format 计数为：

| 类型 / format ID | 候选数 |
| --- | ---: |
| 2D / `0x3420` | 128299 |
| 2D / `0x3431` | 68434 |
| 2D / `0x1131` | 25442 |
| 2D / `0x6432` | 4877 |
| 2D / `0x3430` | 392 |
| 2D / `0x1450` | 320 |
| Cube / `0x3420` | 44 |
| Cube / `0x3431` | 20 |

没有符合这些候选条件的 float、BC6H、array 或 volume 头。这个结果扩大
的是指定安装的主表选择与合格首块覆盖，不把未知路径候选自动命名为
ATEX，不代替完整 TEX/ATEX 验证，不构成其他安装、未索引数据或所有布局
变体的不存在证明。实际 float/array/volume ATEX 的样本边界仍保留。

`tests/fixtures/standard-texture-full-inventory-observations.json` 固定报告
SHA-256 `c2edaff30356f3622daa78c3a760e0af21cb8dd38dded43adde1b21a154e6841`。
报告保存每包主表及观察摘要、完整选择计数、剩余解压预算和候选位置。
它作为库存观察复验，不作为解码成功回归。

```sh
python tools/inventory_standard_textures.py /path/to/game/sqpack \
  /tmp/opencode/standard-texture-full-inventory.json --all-indexes
```
