# Volume 装载地址与块任务证据

## 固定对象与证据等级

对象为版本标签 `2026.10.06.0000.0000` 的 Windows AMD64 客户端，
`ffxiv_dx11.exe` 长度51910400字节，SHA-256：

```text
958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9
```

地址采用首选ImageBase `0x140000000`，偏移以十六进制字节表示，多字节
整数为小端。原件与大型结果位于仓库外，身份绑定在
`tests/fixtures/volume-block-loading-observations.json`；分配器身份另见
`tests/fixtures/volume-allocation-observations.json`，命令引用身份见
`tests/fixtures/volume-command-copy-observations.json`，接口赋值与条件注册见
`tests/fixtures/volume-context-origin-observations.json`。

| 证据目录 | 对象 | 验证层级 |
| --- | --- | --- |
| `volume-tex-handle-10` | 虚表、装载分支、组长度与任务构造 | 独立静态审核，主验证完整复算 |
| `volume-block-job-11` | 块回调、长度槽、队列构造线索 | 主调查静态分析与字节核验 |
| `volume-task-execution-12` | 队列元素、回调参数、所选线程入口 | 主调查静态分析与字节核验 |
| `volume-allocation-13` | 分配器子对象、已有/新区域与元数据写入 | 主调查静态分析与字节核验 |
| `volume-command-copy-14` | 引用整理、lane递归与type11条件参数 | 主调查静态分析与字节核验 |
| `volume-context-origin-15` | 创建设备输出、wrapper赋值与条件TLS注册 | 主验证完整复算；独立选定语义审核 |

10的独立结果共678390字节，整份对象与字节复算相等，无排除字段。
11/12的主验证结果共670648字节，全部复算相等，绑定154项来源；其语义
没有经过独立审核。PE映射复用10独立验证器，不把这种工具复用等同于新
语义的独立验证。分析没有执行目标、附加进程或取得运行时缓冲区。

## Descriptor的两个长度域

10核对45个handle候选槽、23个override及30个新入口，覆盖8357字节代码
窗口与880字节数据；leaf窗口含196字节INT3。完整普通代码按CHAININFO
闭合，异常handler数据只有前缀。

`140303140` 的volume分支按所选起始组组织目标。设相对mip为m，新texture
深度为D，单切片字节数为Sₘ，则指令宽度下：

```text
stored_total = u32(sum(group.u32_at_04))
step_m       = u32(S_m × max(D >> (m & 31), 1))
capacity     = u32(sum(step_m))
```

buffer跨组源指针按 `group+4` 累加；stream按 `group+0` 定位，传
`group+4` 为读取长度。两路目的都按step_m推进。buffer结束处有两个
独立检查：存储累计对stored_total，目的指针跨度对capacity。它们没有
检查各块解码长度之和是否等于完整mip容量。正常完整容量的解释带有效
尺寸及无算术溢出前提；三项descriptor门槛之前还有早退条件。

组内源按u16块span推进，目的按 `u32(block+0x0c)` 累加，每块传mode6
及 `u32(n+15)&0xfffffff0` 容量。组helper本体没有补齐到整mip容量的循环。
完整目的步长与较短块输出之间仅建立条件性地址间隔。

## 任务分派与写入长度

静态mode6模板首qword为 `141d0cb30`。任务T经复制和队列元素传递后，
worker在 `1405e619a` 调用 `[T]`，参数为 `RCX=T+0x48`、`RDX=T`。
回调的 `[RCX+8]` 对应T+0x50的块指针，`[RDX+0x78]` 对应目的地址。
模板位于可写.data；以上映射以相关值保持、成功分支和正常ABI为条件。

块头给出载荷偏移h、压缩字段c及声明输出长度n。回调对n=0跳过写入；
c=32000时经栈buffer复制n字节；其他分支将解码输出放入16 KiB栈buffer，
再按长度槽L复制。L初值16384，wrapper仅在decoder返回1时以输出计数
更新L。callback不检查wrapper返回值，也不读取任务的对齐容量T+0x80。
worker在callback返回后增加完成计数，没有检查解码结果。

错误路径的继续复制是静态条件，不代表安装中观察到解码错误。长度槽、
对齐容量和控制流完成计数都不能独立证明正确输出，更不能定义间隔内容。
字面量 `1.2.5` 与结构大小0x58只是编译产物观察；完整codec等价性及部分
CRT上下文传递没有由这些范围证明。decoder的pdata内含128字节跳转表，
线性反汇编在该范围产生的伪指令按数据处理。

## 分配器与元数据写入

13核对6个完整pdata根、1个转交leaf和1个内部窗口，共3478字节，另有
3个8-byte虚槽。437167字节主验证结果整份复算相等，绑定87项来源、
915行重解码及15个数据流区间；没有独立语义审核。

管理器+0x10为数组第二项A。A的分配入口转交A+0x90的池；在普通u32
容量和16-byte对齐下，大于128字节的请求经池+0x108转交A+8的大块
分配器。三个安装表的+0x10槽分别指向1403377f0、1403358c0、140336ae0。
池的实际比较是 `u32((q-1)>>3)` 对16，不能推广为任意u64大小规则。

已有区域由140336420子分配，成功后还经过140337570。前者的34个显式
内存写指令涉及栈、计数、位图、链表及返回地址前0x40-byte分配头；后者
的9个写指令涉及栈和区域排序元数据。有效区域/链表、元数据不别名payload、
正常ABI和无回绕的对齐算术是排除payload写入的前提。

新区域140337050初始化24个桶头与分配头，底层通过上层虚分配器取得
存储。上层分配器和小块helper的内容语义没有由13确立。局部元数据写入
不能证明实际间隔为零或未初始化，也不能给出缺失mip生成规则。

项目旧fixture的6个范围摘要与固定EXE相符；其中small-or-large旧摘要
覆盖73字节，13完整捕获505字节。72-byte管理器绑定窗口属于更大的函数。
缺失的旧临时报告没有因重新捕获恢复逐条身份。

## 命令引用与条件复制

14的6个完整CHAININFO根和2个相接窗口唯一覆盖7696字节，其中两张内嵌
表共148字节、末尾INT3共10字节，指令7538字节。629303字节主结果完整
复算相等，绑定115项来源和29个数据流区间；没有独立语义审核。

10发布端从TLS+0x238取得Q，在lane0排入16-byte引用，+8为type11命令
指针。batch1402266c0枚举87个lane与步长0x2f78的线程对象，经1402273e0、
14023ba90收集链块并按u32键整理记录。块长0x4000，尾部可含next/NULL
记录；槽计数不能等同于非空命令数。完整排序和复制callee的行为另有边界。

双缓冲表索引为`87*buffer+lane`，指针位于G+0xd8+index×8，计数位于
G+0x648+index×4。batch选择lane30传给140234500；prepass与dispatch
的type19分支通过C+4递归选择其他lane。已捕获的batch内type19目标为31。
TLS对象的注册、batch参数G与全局1428f6780的身份、lane30到lane0的具体
命令链均没有闭合。

dispatch从引用+8取得C，type11选择140234dd2。目的对象bit21、源/目的
格式、目的mip符号和C+0x48决定跳过、直通或转1402301e0。直通调用以
[context+0x17b8]为接口，使用虚槽+0x170；10已写字段使x/y/z为0且source
box为NULL。接口创建/赋值来源与命令可达另有前提，槽号不独立确立API身份。
140240550操作context+0x18所指五项环形状态，不提供接口来源。

两次不同地址的定向查询保留5个代码候选，3个由完整本体确认成call，其余
2个保持候选级；复算只核对保留窗口，没有重复查询。`CORRECTIONS.md`
记录lane-collect的call地址笔误、上下文helper的工作标签、重叠窗口中的
截断字节与两张内嵌表分类。命令传递证据不定义任何缺失像素。

## 创建输出与条件注册

15的5个完整CHAININFO根及6-byte导入尾跳覆盖9850字节。518963字节主
结果逐对象、逐字节复算一致，绑定178项来源、25个数据流区间及34个指令
检查；14的完整629303字节结果亦按封存身份对回。三次不同目标查询各保留
一个候选，确认两条call及一个导入尾跳；验证不重跑查询。

独立审核确认创建参数/输出、wrapper安装、全局接口赋值和条件性TLS规则，
并保留O/G身份、实际注册与lane准备缺口。安装wrapper发生在初始化返回
检查之前；feature-level字段预写0xb000，不能用字段值独立证明创建成功。
主结果中的`independent_semantic_review=false`描述主验证本身，独立审核的
原件与复算另存`review/`，没有把复用PE工具提升为独立语义证据。
独立结果共855119字节，包含59份来源快照、170个指令锚点和8份完整解码
输出；只读入口逐对象、逐字节复算相等，无排除字段。其自有PE映射与主
验证分开，保留整个创建→消费身份链的限定。

PE导入将`14210bda8`绑定到`d3d11.dll!D3D11CreateDevice`。实际创建call
为`140220e95`，经`141794043`尾跳该导入槽。设创建owner为O，三个输出
地址分别为O+0xe0aa8（device）、O+0xe0a90（feature level）、O+0xe0ab0
（immediate context）。调用设置flags=0x80、SDKVersion=7和单元素feature
level数组；HRESULT负值走失败分支。`140240bef`是sampler消费锚点。

上层`140222e30`在创建函数返回非零AL后构造线程数组，首元素地址A写O+8，
步长0x2f78，容量N为输入u32与16的无符号最小值。`1402235d7`的
`lock xadd [O+0x6c],1`返回旧计数i；仅当i<N时，向指定TLS块+0x238写
`A+i*0x2f78`。i没有被证明为0，失败分支也已增加计数；这条注册规则不
独立证明全部线程、收集时计数或实际发布Q的归属。

分配的0x1858-byte wrapper W写入O+0xe0ac0，再传给`140229d90`。该函数
读取G=`[1428f6780]`，将`[G+0xe0ab0]`赋给W+0x17b8。O与G的同一性、
batch参数与该owner的绑定仍缺直接指令边，因此创建→消费COM身份没有
闭合。15的条件赋值证据不回改14封存结果中的历史未知标志。

线程构造器向`141da6150`传Q+0x18、零值和长度0x828；这些是调用参数，
并未提供lane首块/cursor/余槽的完整准备链。`1402216d0`完整本体处理
尺寸/模式状态，捕获标签`owner-queue-prepare`的误称记于15勘误。

Microsoft官方API合同来自`MicrosoftDocs/sdk-api`的固定commit
`656e9656bfaef34dd29cd7ee9e1146aec17e924c`，CC BY 4.0，许可先于正文审阅；
两份未经修改的文档与完整LICENSE保存在15的`specs/`，署名及原URL见
`SOURCES.md`。接口合同不提供游戏对象身份、实际命令可达或缺失mip像素。

## 来源、勘误与验收

10的 `CORRECTIONS.md` 记录独立审核指出的两个长度检查、07引用绑定
与目录guard计数问题；原报告和JSON保持原样。审核中“base通过不等于
完整mip0”的措辞亦有勘误：273个完整base的读取/解码通过，0/273指六级
完整重建，不能混作base失败数。

[分配路径证据](tex.md#volume-上传缓冲区的分配路径)讨论另一项条件：
既有区域子分配不保证payload初始化。它与任务分派不能替代实际装载分支、
运行期表值或间隔内容的证据。原始短TEX的生产转换器、统一mip规则、相位
和前缀writer没有由这条消费链确立。六级完整重建为0/273，缺603624逻辑
字节；[完整单mip读取](volume-mips.md)的独立验收另行成立。

外部目录中的默认验证命令均只读：

```sh
python -B volume-tex-handle-10/verify_archive.py
python -B volume-task-execution-12/verify_chain.py
python -B volume-allocation-13/verify_allocation.py
python -B volume-command-copy-14/verify_commands.py
python -B volume-context-origin-15/verify_context.py
python -B volume-context-origin-15/verify_independent_readonly.py
```

各目录的 `seal_archive.py` 验证精确文件集合及摘要。封存是证据完整性
检查，不提高缺失mip重建计数。工具为自有Python与GNU objdump；没有
咨询新外部实现或引入依赖，不宣称strict clean-room或REA原生Evidence。
