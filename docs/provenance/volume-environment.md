#import "../_components/pele/lib.notc" as pele;

# 天空方向与 AMB 系数证据

## 身份与范围

对象为安装版本 `2026.10.06.0000.0000` 的 Win64 客户端和具名
`bgcommon/nature/sky/ambient/skylight.amb`。EXE 长度 51910400 字节，SHA-256：

`958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9`。

原字节、反汇编、完整报告、独立审核与校验脚本位于仓库外
`../icexiv-data-evidence/volume-environment-input-08/`。地址使用首选 image
base `0x140000000`，范围左闭右开；对象偏移单位为字节。EXE 数值按小端
读取，数字 bit n 的权重为 `2^n`，位编号与字节顺序分开。

[AMB 格式页](../amb/README.md)说明文件布局；本页说明其静态来源、消费
及实际样本关系。来源授权、脚本身份、失败捕获及勘误在证据目录的
`SOURCES.md`。07 两份历史 math 捕获的生产脚本缺席限定继续适用。

## 投影方向

方向状态 G 为地址 `1428fc6e0`，环境对象 B 为 `*(u64*)1428fcf18`。
它们与 renderer、texture context 是不同对象。[逐切片投影](volume-sky-consumer.md#切片采样与数学输出)
接收的 p 来自 `B+0x690`。

图中偏移均为对象内字节偏移；箭头为已核对的静态数据流。

#pele.diagram(r#"""
flowchart TB
    time[Phase time / environment override] --> angles[G: angle update]
    param[B +0x890] --> angles
    angles --> ab[G +0x114: alpha / +0x118: beta]
    rotation[G +0xf0: R] --> matrix[Row-vector matrix transform]
    ab --> matrix
    matrix --> n[G +0xc0: xyz]
    n --> sign[Sign-bit XOR]
    sign --> p[B +0x690: p]
    p --> projection[Slice projection: 27 floats]
"""#)

`140070e00` 对 G 的 `+0xc0/+0xc4/+0xc8` 各 XOR `0x80000000`，写入
B 的 `+0x690/+0x694/+0x698`。矩阵按行存储，helper 计算行向量右乘
`vM`，输入是 `(+0,+0,−1,+0)`；没有归一化、齐次除法或 FMA。
原始乘加顺序以审核 JSON 的逐指令 DAG 保存。

对有限实数角度、理想三角函数，令 H 为小时角、λ 为 G+170 角度转弧度：

```text
α = asin(cosλ · cosH)
β = atan2(sinH, sinλ · cosH)
p = (cosα · sin(R−β), sinα, cosα · cos(R−β))
```

代入后为 `U cosH + V sinH`，U、V 正交且都是单位向量，故是单位大圆方向。
该理想恒等式不证明逐指令 float32 的长度恰为 1。`asinf/fmodf` 有直接
诊断名称链；atan2 的数学行为由完整象限逻辑、241 项表及推导识别，原始
链接符号名未知。没有取得太阳对象的字面命名证据。

角度重算门槛为 `G+0x150==1`。零 delta 仍可能经总秒数重组产生舍入变化。
`+0xf0` 的 setter 对负角度只加 **359°**，对至少360°只减一次360°；
setter及initializer对全局G的实际绑定、G+f0运行值和B+890的上游作者未知。

## 记录装载与时间消费

07 phase、AMB variant 0 loader、variant 1 builder 分别在
`1400651af`、`140083015`、`140083448` 调用相同的记录storage helper。
三者均使用 `0x70` 字节内存记录，前 `0x6c` 为27个系数，末尾为相位。
这是共用结构与操作的证据，没有建立 phase 调用 AMB serializer 的关系。

owner 分配大小为 `0x320`。字段布局：

| 对象范围 | 用途 |
| --- | --- |
| `+0x10..+0x210` | 32 个16字节容器：pointer、u32 count、未解释尾部 |
| `+0x210..+0x234` | 九float H |
| `+0x234..+0x2a0` | 27float 时间混合结果 F |
| `+0x2a0..+0x30c` | 27float 颜色/方向处理副本 P |
| `+0x30c..+0x318` | 三float 归约结果 |
| `+0x318` | u16 缓存索引 |
| `+0x31a/+0x31b` | 更新与处理状态 |

`140070010` 返回 `0x320+0x70×Σ32个u32 count`，是内存尺寸。
variant 0 从文件复制相位与H；variant 1从ID目录选择记录，构造相位
`f32(u32(86400×i))/f32(count)`，并依selector scalar选择三通道缩放。
分子在 i=49711 首次 u32 回绕；该机器行为不能当作文件 Count 的取值规则。
只有 scalar 有序等于1.0时跳过缩放，连全局通道gain也一并跳过。

时间consumer从第二参数 XMM1 接收 t，count 只取容器字段的 **低u16**。
在有效、有序、单周期相位的普通路径中，它选择相邻记录并进行周期插值。
27 个系数的混合操作为分别舍入的
`f32(f32(f32(1−w)×A)+f32(w×B))`，没有FMA；还可按 B+20 将旧F与结果
作第二次混合。binding导出 `owner+0x234` 的F。

F复制到P之后，P接受同槽三通道颜色变换、可选方向注入、六方向权重及
整体缩放。H被变为二次型矩阵；原指令中的 M01/M10 系数不同，M11使用
`−0.511664`，不能替换为熟悉的对称照度矩阵。完整标准余弦卷积、颜色
物理单位和最终shader等价性没有得到证明。覆盖分支只更新P，binding也
仍导出F；它不保证每次调用都生成新的F。

构造、storage与consumer的静态失败路径没有提供通用输入安全保证：分配
失败策略未知，phase不检查storage结果，非空pointer且低u16 count=0
没有独立保护。这些是客户端行为证据，不是本库的解析契约。

## 安装样本

资源身份链由 `.amb` key `0x00616d62`、factory表 `14214fca0`、factory
`140324f10`、handle表 `14214fb38` 及其 `+0x108→140324e10` 确认。
factory请求 `0xc8` 字节。`[140065300,140065314)` 将具名路径传给资源
请求入口；间接magic为 `1428a1390→1421186e4="AMB\0"`。

Rust与两份独立Python观察核对同一安装entry的51块解包，全部806544字节相等。
解包结果SHA-256：

`9b97e1c55a1268cc2491a91c0771ca37d9758196b9092a6082270e880335d919`。

variant 1目录有592项，系数区有7424条记录，包含200448个有限float32。
目录 `[16,4752)`，系数区 `[4752,806544)`；该样本引用恰好连续覆盖全部
记录，没有重叠、空洞或尾部。Count=1/2/24的项数为294/1/297；唯一
Count=2是ID105，其两条108字节系数记录完全相同。

592个ID/count与EXE天空表逐项相等；非空天空表中缺少AMB目录项的只有
123与392，03指定主索引也没有命中二者。该范围不表示全安装确认缺失。
与03的452条volume cohort关联为：

| TEX depth | AMB count | 条目数 |
| ---: | ---: | ---: |
| 2 | 1 | 178 |
| 2 | 2 | 1 |
| 24 | 24 | 125 |
| 32 | 24 | 147 |

451条匹配包含179个完整条目、272个短条目。`sky_400` 没有native ID，
单独列为cohort中的第273个短条目。其余141个AMB项由封存03分类确认：
1个4×32×2的6 mip volume、25个8×32×24的1 mip volume、16个1 mip
及99个6 mip的8×32非volume。TEX深度、目录Count与phase记录数分别验收。

## Base 与零阶系数的必要条件

仓库外 `volume-ambient-bounds-16/` 对451条关联TEX、6708条AMB记录的
20124个零阶颜色通道作有界比较。检验前提是对象输入等于指定TEX base、
记录i对应切片i、投影正常完成，并将系数原样写入同ID的AMB记录。

取样列依赖方向p；零阶基为常量不使输出独立于p。对每个纹理行取全列
颜色最小/最大值，再按原504次采样顺序做向外binary32舍入，可以包含
所有合法列选择及四种IEEE舍入模式，无需拟合方向。两种颜色分支分别是
字节归一化与归一化后平方；包络涵盖每步舍入选择，非零运算值均为normal。

不平方分支有16381个通道低于下界，排除5494条记录、441条资源；平方
分支的20124个通道均位于区间内。8133个通道的各行在八列上恒定。
例如sky_014切片0的R字节全为10，实际AMB值仍严格低于不平方下界，
无需假设具体方向就构成该分支的反例。

列与颜色通道分别取界包含不可共同实现的选择，区间内不证明存在对应
方向或真实生产调用。平方开关未知时，没有资源同时排除两个分支；结果
不确认平方开关值，也不确立生产器、对象base身份或后续写出关系。

主比较用整数尾数/指数，独立审核用精确整数格与二分舍入，完整9793096
字节结果逐字段/字节相等，无排除字段。独立结果、5636项舍入边界检查、
原指令与输入身份核对见 `review/REVIEW.md`；固定引用见
`tests/fixtures/volume-ambient-bounds-observations.json`。

## 序列化与文件输出

`1400830c0` 写variant 0，`140083620` 写variant 1。两者都以 `wb` 打开
调用者提供的路径，写公共头、count/目录及系数记录。variant 1 的文件
`+0x0c` 在写出链中控制 M 个 `0x6c` 记录的循环；`+0x08` 的u16含义未知。

换端条件是调用参数完整int32等于1；tag只保存其低8位。HHI块按两个u16
和一个u32分别换端，系数按各float32位模式换端，不能把整块当u64反转。
参数257与1的低8位相同，却不进入同一换端分支；任意参数都有效的契约不成立。

所选文件helper链抵达PE导入 `CreateFileW`、`WriteFile`、`CloseHandle`。
`wb` 路径产生写权限 `0x40000000`、creation=2、share=0，并选择binary
非text数据路径；属性初值 `0x80` 可受umask变为1。窄转宽及部分CRT分支
未完整审核，helper的行为型名称不等于原链接符号名。

两个serializer仅检查打开所得stream非空，忽略write和close返回值，最终
成功标记不能证明所有字节写入成功。record只写AL=1；table写EAX=1后经
未捕获的cookie用途helper。所选文件链以路径转换成功、descriptor有效及
未捕获callee正常返回为条件；stream buffering会改变指针和分块，一次
serializer write不必对应一次WriteFile。两入口的exact-u64和普通disp32-ending
查询无候选；这不证明它们不执行。实际phase→serializer调用、真实AMB的
生产来源及TEX前缀writer都没有由这条文件IO链确立。

## 文件打开入口的调用者

对已确认的 `141de08a8` 作一次直接 E8/E9 查询，得到16个真实call、11个
调用者，全部从 `.pdata` 入口解码确认；没有截断或重复旧入口负向扫描。
原件与审核保存在 `../icexiv-data-evidence/volume-texture-writer-09/`。

纹理导出wrapper `1402320a0` 在其条件分支调用 `140232260`。后者构造
`BM` 标记、14字节文件头及40字节图像头，bitcount=24、数据偏移54，
随后倒序逐行把四字节源像素转换为三字节输出。这些字段和写出长度确认
其为24-bit BMP路径；虚表 `+0x50` 是描述查询槽，不是80字节TEX头。
行padding的显式初始化没有出现在该函数，实际填充值未知。

另两条编码表路径写256条固定记录，每条按1、4、4字节分别写出，共2304
字节。一条使用调用者传入stream，其打开模式不在该函数确定。结构支持
Huffman表解释，名字仅作旁证。其余文本模式调用者只审到打开前缀。

该路线覆盖12个函数入口、4个完整body、8004字节代码（含前缀重复），
没有取得TEX前缀或逐mip写出证据；其他API与间接调用不在范围内。
完整验证结果及148文件集合复算相等，固定引用见
`tests/fixtures/volume-file-output-observations.json`。

## 验证与验收

根校验覆盖60个捕获、41034字节（含重叠）、47个 `.pdata`/CHAININFO组。
方向审核的v1/v2完整重算结果分别对回封存JSON，仅排除两个采集时间字段；
输出审核完整结果仅排除utc；样本审核逐字节比较整份JSON。
序列化审核覆盖30个函数根、33段、8825字节及82个opcode固定点，完整
结果复算不排除字段。显式span中16字节填充helper的version 2 unwind与
两次失败记录均保留。

固定摘要及引用见 `tests/fixtures/volume-environment-observations.json`。
运行期方向、生产转换器、统一mip数值规则及TEX前缀写出关系各有独立验收
要求；六级完整重建为 **0/273**，缺失 **603624** 逻辑字节。
