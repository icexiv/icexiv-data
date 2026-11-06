#import "../_components/pele/lib.notc" as pele;

# 天空 CPU 投影与纹理缓存证据

## 对象与证据身份

本页记录 Win64 客户端的天空请求、纹理 CPU 缓存和逐切片系数计算。
目标安装版本为 `2026.10.06.0000.0000`；EXE 为 51910400 字节，SHA-256
`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`。
原始代码、反汇编、独立审核及大型报告位于仓库外
`../icexiv-data-evidence/volume-sky-consumer-07/`。

地址为 image base `0x140000000` 下的首选 VA，范围左闭右开，对象偏移单位
为字节。数字按小端读取；bit n 指整数权重 `2^n`，与字节顺序分开。
静态数据流、数学识别和运行期未知项分别记录。

## 天空请求与 CPU 缓存模式

图中 `+` 后数值均为对象内字节偏移；箭头表示满足各分支条件的静态数据流。

#pele.diagram(r#"""
flowchart TB
    req[Sky request: u64 at +8 = 3] --> factory[TEX factory: 1403082b0]
    factory --> handle[TextureResourceHandle +0x18]
    handle --> mode[bit 1: mode]
    mode --> create[Volume creation: bit 11 or bit 12]
    input[Non-null BGRA8 initialization input] --> copy[Byte-preserving copy]
    create --> copy
    copy --> cache[Texture +0xa8: CPU mip records]
    cache --> map[Object mip 0: slice sampling]
    map --> coeff[R / G / B: nine floats each]
"""#)

### factory 身份与新建条件

`14007f557 → 1402feae0 → 140316090` 传递天空请求。默认 16 字节的 u32
为 `[0,0,3,0]`，故 `request+8` 的 u64 为 3；请求 `+0x24` 为零。
`.tex` 后缀得到 key `0x00746578`，注册代码 `14031b4e9..14031b504`
关联 factory 虚表 `14214d848`。查找函数 `14031c5d0` 在匹配、非空的
factory 上尾调用虚表 `+8`，该槽为 `1403082b0`。

该 factory 分配 `0x150` 字节，调用 `1402fee60`。基类构造器
`1402feeac/1402feeb0` 将 `u64(request+8)` 复制到 `handle+0x18`。
factory 安装的 `14214d6e0` 虚表同时关联：

- `+0x80 → 1403027c0`，返回 `Client.System.Resource.Handle.TextureResourceHandle`；
- `+0x100 → 140302b00`，包含对 texture 创建路径的调用；
- `+0x108 → 140304800`，包含 staging 初始化路径。

因此，成功注册、分配的新建请求可将 bit 1=1 带到创建入口；没有由静态
代码观测运行时注册表或全生命周期字段值。已有 handle 的合并函数只有在
state=3、请求非空且 `request+0x24≠0` 时复制 `+8→+0x18`。天空请求的
`+0x24=0` 不满足该覆盖条件，不能将新建路径的 bit 1 推广到已有 handle。

### mode 传递

`140305240` 取 handle `+0x18` 的 bit 1，作为第七参数送入
`140305bd0`。该函数的初次及重试 volume 分支均将 mode 作为第五参数传给
`1403064d0`。后者计算 `(mode+1)<<11` 并传给 `1402247b0`。

在 8×32、属性 `0x01000000` 及有效 selected_mip 的既定创建分支中，mode
0/1 的 texture flags 分别为 `0x05000804/0x05001004`。另一创建分支的
`0x01000804/0x01001004` 具有相同 bit 11/12 选择。完整描述参数见
[存储与创建证据](volume-generation.md#texture3d-的创建契约)。

## CPU map 与缓存字节来源

`140217230` 的必要条件是 `(flags&0x13c00000)==0x01000000`、bit 11=0、
texture `+0x68≠0`。bit 13 的分支优先于 bit 12，只分配一个 slice 的
`+0xb0` 缓冲。bit 12 从 `+0xa8` 表取得 mip 记录。default 分支分配
`+0xa0`，bit 15 可以改返 `+0xa8`。

两个记录均为 16 字节，但字段次序不同：

图坐标为 bit，分别以各记录起点为零；字段数值仍按小端读取。
`+a8/+a0` 表项：

#pele.diagram(r#"""
packet
0-63: "Pointer: u64"
64-95: "Row pitch: u32"
96-127: "Slice pitch: u32"
"""#)

map 输出及 `+b0`：

#pele.diagram(r#"""
packet
0-31: "Row pitch: u32"
32-63: "Slice pitch: u32"
64-127: "Pointer: u64"
"""#)

`14030484f/140304856` 从 handle `+0x148/+0x130` 取 staging/texture，
在指定分支经 `14030485d → 140216dc0 → 140218390` 初始化。非空输入的
BGRA8 `0x1450` 路径跳过转换，bit 12 路径将输入原样复制到 `+a8`。
空输入、bit 15 的路径也能分配该表，故表非空不能独立证明像素来源。

普通创建调用与特殊 staging 初始化各有 descriptor、状态和虚调用门槛；
两段静态链不独立证明同一次天空加载的调度次序或实际缓存输入。

令 D 为对象深度，S_m 为对象 mip m 的 slice pitch：

```text
缓存分配及复制：u32(S_m × D)
输入指针推进：  u32(S_m × max(D >> (m & 31), 1))
```

复制长度使用未右移的 D，源推进使用缩减深度。有效 D≥1、m=0 时相等；
m>0 时不能统一改写为同一式。此处仅确立客户端的跨度计算，不证明源缓冲
足够长或缺失像素是什么。字节复制函数的四张 RVA 表和 rep 分支没有生成
像素；rep 分支以 ABI 的 DF=0 为前提。

mode 0 被 map 的 bit 11 门槛拒绝；mode 1 可取缓存，还需 flags 保持、
资源有效及非空成功初始化。slice 消费者请求对象 mip 0。将其认作原 TEX
base 还需 selected_mip=0、输入字节来源及缓存状态证据。

## 切片采样与数学输出

`140064cb0..140065133` 的完整七个 chained 代码区段共 1155 字节。对正
尺寸、有限输入向量 p、无整数溢出的普通路径：

- 层 j 从 `1−H` 到 `H−1`，每层八个方向，权重为 float 运算的 `1/(16H−8)`；
- y 由 j 除以 `max(H−1,1)`，半径由 `sqrt(1−y²)` 得到，方位角为 k×45°；
- 忽略舍入的方向为 `(r sinθ,y,−r cosθ)`；行号为 `H−1−|j|`；
- 列号按方向与 p 的点积映射、加 0.5、floor、钳制得到；p 未在该函数归一化；
- ±j 读取同一行，但 p.y 可以使其列号不同；极点各重复八次计权；
- 像素 +2/+1/+0 分别除以 255，可选逐通道平方，按 R/G/B 累加；alpha 不参与。

cosf/sinf 的身份来自直接诊断名称链；floor 入口的完整 124 字节位运算
证明有限 binary32 的 floor 语义，原始符号名未知。三角函数分派和 MXCSR
实际值没有运行期观察。

九槽多项式为 `1,y,z,x,xy,yz,3z²−1,xz,x²−y²`；字面常量的 binary32 bits：

```text
3f906ec1 3ffa2a2c 3ffa2a2c 3ffa2a2c 408bd89d
408bd89d 3fa17b0f 408bd89d 400bd89d
```

自推导的调和性和球面积分支持其为近似统一四倍缩放的实球谐基。常量与
四倍精确归一化量的最近 binary32 仍有 −4…+16 ULP 差异，不能替换常量
声称位精确重放。36 条浮点运算按原指令分别舍入：槽 6 先 z×3 再乘 z，
槽 8 的平方分别舍入后相减。输出 R/G/B 各九 float，各占 `0x24` 字节。
AMB 装载、时间混合及方向处理见[环境系数证据](volume-environment.md)；
标准照度卷积及物理单位需要独立数值证据。

逐相位函数 `140065140` 构造 stride `0x70` 的记录：前 `0x6c` 为系数，
末尾 float 保存分别乘除得到的 `i×86400/K`，K=0 时跳过循环。它临时更新
环境 B=`*(u64*)1428fcf18` 的 `+0x10`，以 B+0x690 作为 p。可选字段能
覆盖环境输入，故记录末尾不必等于被更新环境使用的输入。
[方向审核](volume-environment.md#投影方向)确定 p 的角度来源、行向量
矩阵顺序及理想单位大圆形式；[记录审核](volume-environment.md#记录装载与时间消费)
确定共用容器与 AMB 消费结构。phase 实际入口及其与序列化器的生产调用
关系未知。

## 来源与验证范围

数学与访问审核自行解析 PE、核对原始字节/opcode，使用 GNU objdump 离线
重解码。访问审核包含 13 范围、11456 字节、2765 指令边界、127 组 opcode；
数学审核保留完整常量、36 条运算和函数身份链。slice 根 unwind 的旧 20
字节捕获只到 handler RVA，完整 24 字节记录及 handler 数据补证分别保存。
其余捕获的 EH 前缀不冒称完整 handler 数据；version 2 捕获失败亦保留。

math/access 独立复验为带来源限定的通过：原始字节、方程和统计复算一致，
math 两份历史捕获的旧生产脚本源码未保存在所审封存树中，其摘要与封存脚本
分别登记。固定八方位采样不因增加 H 就保证任意函数的球面积分收敛。
请求链复核包含 26 个捕获、166 个指令锚点与 4 个虚表槽；名字的 51 字节
捕获仅含字符本体，终止 NUL 另从同一 EXE 核对。

请求链的厂商程序片段仅供离线数据流研究，原件留仓库外。没有将 CRT 或
FFXIV 实现移植到本库；沿用既有来源与许可记录，不宣称严格 clean-room。
固定引用见 `tests/fixtures/volume-sky-consumer-observations.json`。

这些证据确立消费计算和缓存条件。生产转换器、滤波精度、抖动相位与前缀
writer 的身份未知；六级完整重建为 **0/273**，缺失 **603624** 字节。
