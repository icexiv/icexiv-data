#import "../_components/pele/lib.notc" as pele;

# PBD 标量比值的 shader 消费证据

## 具名消费与证据范围

指定 Win64 客户端将角色／模型 PBD 标量比值写入名为
`g_ModelParameter` 的参数对象。七份固定 SHPK 中的 316 个 vertex shader
将它反射为 16 字节 `ModelParameter`，含 byte 0 的 `m_Params: float4`，
并各读取一次 `m_Params.x`。读取值进入与
`g_InstanceParameter.m_Wetness` 相连的四条相邻运算。

令 `R` 为这个比值、`Y` 为指令所用位置的 y 分量、`W` 为反射命名的
`m_Wetness: float4`，运算顺序的代数摘要为：

```text
wet = min(max((Y * R + W.y) * W.x, W.z), W.w)
```

MAD、MUL、MAX、MIN 的操作码、分量和前后依赖有逐项字节证据。上式不规定
shader 浮点精度、MAD 舍入或 NaN 行为，也不是 shader 执行所得数值。
这些名字和运算支持“比值参与基于位置 y 的湿润度计算”的语义推断。
`character.shpk` 的 VS2 将结果写到 `TEXCOORD4.w`；VS48 的条件路径将其
用于 `m_Wind` 向量的衰减，数据依赖继续到 `SV_POSITION`。

消费比值不能唯一确定 PBD 原始标量的物理定义或单位。绝对身高、包围盒
尺寸及统一骨缩放均不能仅由这条链命名。完整变形空间和 EPBD 策略的证据
边界见 [PBD 来源记录](pbd.md)。

## 输入、来源与坐标

- 授权安装版本文本为 `2026.10.06.0000.0000`。Win64 executable
  `ffxiv_dx11.exe` 长 51910400 字节，SHA-256 为
  `958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`。
- 七份 Win32 DX11 SHPK 的提取身份沿用[材质来源](materials.md)，各完整文件
  SHA-256 固定在 `tools/observe_pbd_scalar_shaders.py` 的 `INPUT_DIGESTS`；
  下文见证另固定其 DXBC 摘要。
- native 地址是 PE 首选 VA，image base 为 `0x140000000`，区间左闭右开。
  对象 `+offset` 以各对象起点为原点，单位是字节。shader 文件偏移以整个
  SHPK 起点为原点；`SHEX+offset` 以 chunk payload 为原点；RDEF 字段偏移
  以变量或结构体起点为原点。PE、SHPK、DXBC 数字字段按小端读取。
- native 观察使用 GNU objdump 2.46；REA 原生引擎限制沿用
  [既有记录](cmp.md)。本页是离线字节／指令观察，没有运行目标或 shader，
  也没有 REA Evidence。完整反汇编、资源与逐 shader 报告保存在仓库外。
- PBD 查询与角色／模型 ID 来源依据 [PBD 静态证据](pbd.md#win64-标量与关系的离线静态证据)。
  SHPK 元数据的独立 wire 观察参考本库 `tools/verify_materials.py`；
  没有咨询新的第三方 FFXIV 解析实现或读取 `legacy/`。既有实现审阅接触
  记录保留，不宣称严格 clean-room。

### D3D token 声明的许可审查

Microsoft `DirectXShaderCompiler` 固定 commit 为
`ea3489109cc8a968b36e36ca20a698a68b516d32`。先审根许可证，再阅读声明头：

| 来源 | SHA-256 |
| --- | --- |
| [LICENSE.TXT](https://github.com/microsoft/DirectXShaderCompiler/blob/ea3489109cc8a968b36e36ca20a698a68b516d32/LICENSE.TXT) | `27a49e35d1da96eba18fba54bc882667ff0ff8c0254f16f2b6e165d605ba7df8` |
| [d3d12TokenizedProgramFormat.hpp](https://github.com/microsoft/DirectXShaderCompiler/blob/ea3489109cc8a968b36e36ca20a698a68b516d32/include/dxc/Support/d3d12TokenizedProgramFormat.hpp) | `cf808b363406fce9387d7ec8daec19940175af966589c0d3e084a4c41c3f5060` |

声明头标明 Copyright (C) Microsoft Corporation、University of Illinois
Open Source License；根许可证包含 University of Illinois/NCSA 条款。
完整头、根许可证、版权与免责文字保存在外部研究目录
`/tmp/opencode/pbd-scalar-consumer-research-01/`。

阅读范围为 shader version、opcode enum、instruction length、operand type、
component mask/swizzle/select、index representation 和扩展 operand modifier。
观察器从外部固定头读取 opcode 声明；没有编译 DXC 或引入其实现依赖。
RDEF 的 24/32/40/36/12 字节记录形状与 RD11 标识、ISGN/OSGN 的二十四字节
记录由固定样本检查并保留原始 words，不泛化为全部 DXBC profile 的解析器。

## Native 名字、参数字节与 packet 链

### 比值来源和首个 f32 写入

| 首选 VA | 可观察操作 |
| --- | --- |
| `140442100`、`140442117` | 两次直接调用 PBD 标量查询 `1417fa2d0`，第一次值保留在 xmm7、第二次在 xmm6 |
| `14044211f`、`140442128` | 第二次除以第一次，保存到角色对象 `+0xc68+4*slot` |
| `140442855`、`140442880` | 按模型槽读取比值，尾跳参数写入器 `14042c740` |
| `14042c7b7` | 从 utility `+0x410+8*thread_index` 取参数对象 |
| `14042c7c6`、`14042c7d0` | 调用 `14021f3a0`，offset=0、size=16、第四参数=2；成功时写返回地址的首个 f32 |
| `14042c7db..14042c7e9` | 读取 utility `+0x530` 的索引，将参数对象存入 context `+0x940[index*8]` |

普通角色槽分支中的比值是角色 ID 标量／所选模型 ID 标量；特殊槽的 ID
来源仍按原证据区分。这里直接确认首四字节写入，剩余十二字节没有零值推定。

### 名字登记与动态索引

`140425fb4` 从全局 `1428f6780` 取得 manager。
`14042629a` 的 RIP-relative 引用指向 `142166cb8`，对应文件 offset
`0x21654b8` 的完整字符串 `g_ModelParameter\0`。随后的
`1404262a1/1404262a5` 将 manager `+0x50` 和这个名字交给 `140236a30`，
`1404262aa` 将返回索引保存到 utility `+0x530`。

`140236a30..140236d26` 包含字符串长度、hash、完整名字比较与登记索引
分配；`140236b50..140236b68` 的比较到终止零。因此 `+0x530` 保存的是
名字注册表的动态索引。它与 SHPK CRC `0x4e0a5472`、GPU b-slot 是三个
独立坐标，不能直接用一个数替代另一个。

### 参数对象与 staging 指针

`140426024..14042605b` 对 utility `+0x410` 起的十六个对象调用
`140226100(size=16, flags=2)`。factory 在 `140226197` 调用
`14021eec0`，后者在 `14021ef49/14021ef4c` 保存对齐长度和 flags。

`14021f3a0` 的 flags=2 路径由 TLS index `142c208c0` 和 TLS block
`+0x238` 取 context，在 `14021f424` 请求 16 字节 staging 空间；
`14021f448` 写对象 `+0x34=16`，`14021f474` 写对象 `+0x28` 为返回的
staging 指针。该路径不进入 `0x4000` 的另一分支。这将首个 f32 的写入
地址与 packet 读取的同一对象 `+0x28` 关联。

### SHPK 资源名到 shader descriptor

`1417ef690` 的入口检查 `ShPk` 与版本 `0x0e01/0x0d01`，从头部
`+0x10/+0x14` 建立 code base 与 names base，从 `+0x58/+0x54` 定位
不同版本的 shader 元数据。VS 循环 `1417f0263..1417f02ca` 按二十字节
shader 记录及十六字节资源记录推进；`1417f027c` 将 stage=0、该记录、
资源表、code base 和 names base 交给 `1417eee90`。

该构建器使用同一全局 `1428f6780` 的 manager。`14021bb60` 返回 shader
`+0x28` 的 descriptor 表；每条 constant resource 的名字交给同一
manager `+0x50` 和名字登记函数 `140236a30`。返回动态索引写入八字节
descriptor 的 `+4`；文件资源 `+0xc` 的 slot 低字节写 descriptor `+0`，
资源 `+0xe` 的 size 写 descriptor `+2`，descriptor `+1` 清零。
相应写入位于 `1417ef088..1417ef0b8`。

### 相同 TLS 来源到 packet 读取

scalar writer 在 `14042c75b..14042c789` 使用 TLS index `142c208c0`
和 TLS block `+0x238`。packet caller `140244a20` 在
`140244a33..140244a6b` 使用相同 index 和字段，并于 `140244c40`
传该 context 给 `14023c200`；后者在 `14023c649/14023c65d` 将其传给
`14023c760`。context 来源由这条调用链关联。

`14023c760` 遍历 shader `+0x28` 的 descriptor，以 `+0x40` 为数量：
`14023c7a0` 读 descriptor `+4` 的名字索引，`14023c7ab` 据此读取
context `+0x940[index*8]` 的参数对象。`14023c7b6..14023c7c7` 把该对象
`+0x28` 的数据指针、`+0x34 >> 4` 的长度和名字索引写入十六字节 packet
记录。长度在这里以 16 字节为单位。

#pele.diagram(r#"""
flowchart TB
  ratio["character scalar / model scalar"] --> bytes["parameter byte 0: f32"]
  name["g_ModelParameter name registry"] --> slot["context object at dynamic name index"]
  bytes --> slot
  resource["SHPK resource name and b-slot"] --> descriptor["shader descriptor: same name index"]
  name --> descriptor
  slot --> packet["packet: data pointer and length"]
  descriptor --> packet
  resource --> reflection["RDEF: ModelParameter.m_Params.x"]
  reflection --> wetness["SHEX: position y and m_Wetness"]
"""#)

图中箭头表示静态字段与名字关系，不表示一次 draw 的已观察调用顺序。
GPU API 提交末端、实际绑定和中途覆写没有逐级运行证据。

## 七份 SHPK 的有界检查

| SHPK | VS 总数 | PS 总数 | 命中该资源名的 VS |
| --- | ---: | ---: | ---: |
| bg | 56 | 3850 | 0 |
| bgcrestchange | 9 | 12 | 0 |
| character | 176 | 1038 | 124 |
| characterlegacy | 64 | 1740 | 24 |
| hair | 192 | 200 | 144 |
| skin | 72 | 384 | 24 |
| water | 19 | 777 | 0 |
| 合计 | 588 | 8001 | 316 |

这七包的 HS/DS/GS 数量均为零。8589 个 shader 的资源表逐个遍历，名字
命中只在上述 316 个 VS；每个命中项检查完整 DXBC 边界与 chunk 非重叠、
SHPK slot 与 RDEF bind point、一条 16 字节 `ModelParameter` 及其 byte 0
的 `m_Params: float4`。

SHEX 全部指令边界与操作数均展开，递归相对索引操作数也在检查范围。
所有 CB 绑定号为直接索引；每个 VS 对该 CB 恰有一次非声明读取，读取
`[0].x`，并进入前述四指令链。`g_InstanceParameter.m_Wetness` 的类型
为 float4，位于 CB byte 64，即元素 `[4]`。四条运算没有 saturate 或
扩展 instruction modifier，相关源操作数为无 modifier 的直接读取。

- 252 条链使用 `v0.y`；ISGN 将该寄存器命名为 `POSITION0`。
- 64 条链使用 `r0.y`；下节只对其中一个见证展开完整位置来源。
- 最后一条 MIN 有 140 次直接写输出寄存器，OSGN 均对应 `TEXCOORD4.w`；
  另 176 次写临时寄存器。

8001 个 PS 的指令不在此检查内。“无同名资源”不排除 PS 通过插值输入
间接消费结果；七包以外的 SHPK 也不属于该扫描集合。

## 两条下游见证

两者来自 `character.shpk`，完整 SHA-256 为
`c2453552ce2527f1f92a9f62d04d2c5fbed67226306629826edc0c1d9c646abe`。
VS index 从文件 vertex-shader 表的首项起按 0 编号；下列 offset 均为
SHPK 文件字节偏移。

### VS2：POSITION0.y 到 TEXCOORD4.w

DXBC 位于 `0x45495c`，SHA-256 为
`cd16bc141dab52cac8d269f73be4d1e61cf8b340e41ed5c548a7ea252d4f8dd6`。
RDEF payload `0x454998` 将 `g_ModelParameter` 绑定描述为 b3、
`g_InstanceParameter` 为 b2；`m_Params` 与 `m_Wetness` 的 byte offset
分别为 0 和 64。ISGN payload `0x4550f4` 将 v0 命名为 `POSITION0`，
OSGN payload `0x4551f0` 将 o6 命名为 `TEXCOORD4`、o0 为 `SV_POSITION`。

四条运算位于 `0x455878/0x4558a4/0x4558c4/0x4558e4`，最后写 o6.w。
这个见证的 77 条指令中，scalar 的该值流没有改写 o0。

### VS48：法线偏移后的位置与风位移

DXBC 位于 `0x4aa880`，SHA-256 为
`cd65f2a45d9f5a78af4ea95e537fd7ea3d47163ea92a63d59dff311ebaff5401`。
RDEF payload `0x4aa8bc` 将 b5 命名为 `g_ModelParameter`，b4 为
`g_InstanceParameter`；b4[4] 是 `m_Wetness`、b4[5] 是 `m_Wind`。
ISGN payload `0x4ab290` 中 v0=`POSITION0`、v3=`NORMAL0`、v2=`COLOR1`。

`0x4ab47c..0x4ab4c4` 先形成
`P' = POSITION0 − NORMAL0 × (g_MaterialParameter[18].w × 0.01f)`。
`0x4ab59c..0x4ab628` 使用 `P'.y` 计算湿润度，所在分支条件为
`COLOR1.x² × g_MaterialParameter[12].y > 0`。

`0x4ab9b0` 用具名 `m_Wind.xyz` 形成向量 `V`；
`0x4ab9d0/0x4ab9ec` 的代数摘要为 `V × (1 − min(wet, 0.7f))`，
其中 0.7f 的原始位为 `0x3f333333`。随后
`0x4aba14..0x4aba98` 计算长度、带 epsilon 的归一化和限幅，
`0x4abab4` 将位移加入位置；`0x4abaec..0x4abbe4` 的矩阵运算到达
OSGN payload `0x4ab384` 声明的 `SV_POSITION`。

这是条件性顶点位置数据依赖的静态见证，反驳“全部分支直接使用原始
POSITION0.y”及“比值完全不影响顶点位置”的概括。具体输入还受分支、零值
和限幅影响，不由此断言每次绘制产生可见位移。其余材质常量也没有仅凭
数组位置赋予业务名称。完整 81 条指令、token words 和反射记录保存在外部
`character-VS-048.json` 与 `.tokens.txt`；VS2 有对应的 `002` 文件。

## 复验与未决输入

`tests/fixtures/pbd-scalar-consumer-observations.json` 固定输入身份、报告
摘要、计数及两个 DXBC 见证。native 观察器检查 15 个程序区间、14 条直接
call/jmp、9 条 RIP-relative 引用、20 个字节锚点和四项具名字节；shader
观察器检查上述 316 条四指令依赖链。摘要验证固定字节与观察结果身份，
人工语义解释仍需按所列地址与数据流复核。

| 固定报告 | SHA-256 |
| --- | --- |
| native `report.json` | `78974904dd9d3941814632e08b170a74e9737de57abba1364e051f8d9afef927` |
| shader `report.json` | `b59828af01757f578e109bf29574ae6533503d073160cc2edfa020b550ef394a` |

```sh
python -B tools/observe_pbd_scalar_native.py /path/to/game/ffxiv_dx11.exe \
  /tmp/opencode/pbd-scalar-native-check

python -B tools/observe_pbd_scalar_shaders.py \
  /tmp/opencode/render-regression-suite-39/inputs \
  /tmp/opencode/pbd-scalar-consumer-research-01/d3d12TokenizedProgramFormat.hpp \
  /tmp/opencode/pbd-scalar-shaders-check
```

输入目录也可提供相同 SHA-256 的七份提取文件。输出目录必须不存在且位于
仓库和输入目录之外。固定结果使用 Python 3.14.8 / GNU objdump 2.46；
`execution.json` 单独记录绝对路径、工具身份与脚本摘要，避免环境路径
改变语义报告的摘要。原始数值 token 的文本显示同时保留 binary32 解释与
bits，整数操作数的类型应根据 opcode 和 bits 判断。

未决问题及所需独立输入：

- 原始 PBD scalar 的物理定义：带命名和单位的资产／制作规范或独立对照；
  比值、种族编号和近似身高不能唯一确定它。
- `m_Wetness` 四分量的 native 生成、状态选择，以及其他 VS 的完整间接
  输出依赖：相应写入路径与选定 shader 的有界数据流证据。
- 特定绘制的 pass/key、permutation、参数覆写与视觉结果：带 VS/PS 身份、
  绑定 CB 字节、相关顶点输入和选择状态的合法既有离线捕获。新的运行捕获
  需要独立授权。
- 完整 source/target/bind 空间、缓存别名与 EPBD 优先级：继续遵循
  [矩阵消费与请求来源](pbd.md)的证据边界，不能用本页的标量消费链替代。
