# 实资源变异安全验证

## 输入与方法

输入为用户授权的 Win32 安装 `2026.10.06.0000.0000` 中已固定的资源。
`verify_mutation_safety.py` 选取完整联验生成的 28 个提取文件与一份真实
index2 容器快照；每个资源及 EXD 所需 EXH 的 SHA-256 保存在外部报告中。
其有序输入列表的摘要由 `mutation-safety-observations.json` 固定。原始
资源和派生变异不写入仓库或安装目录。工具由本项目编写，没有引入额外
解析库、模糊测试依赖、第三方实现代码或专有资源再分发许可。

29 个种子覆盖 17 类解析入口：MTRL、MDL、SHPK、TEX、STM、EQP、CMP、
PBD、EQDP、IMC、SGB、AVFX、SKLB、PAP、EXH、EXD 和 index2。Havok 对象图
通过 SKLB/PAP 入口解析。种子包括两种 SHPK 版本、16/32 行材质、实际
浮点 TEX、BC6H 零 cube、其他 cube/array/volume，以及含 spline 的 PAP。
ATEX 的独立文件种子、主 index、DAT 后端故障与全部查询路径不属于本组
覆盖；对应的正向资源验证和受控单元回归分别记录。

`validate_mutations` 对每个种子按固定顺序执行：

- 原件预检；解析必须接受，否则检查失败。
- 从长度 0 到 `min(length,128)-1` 的短前缀，及四分之一、一半、四分之三、
  末字节之前的截断。
- 首 256 字节中按四字节对齐位置替换 u32 为 `0`、`0xffffffff`、
  `0x80000000`、`0x7fffffff`。
- 256 次确定性 XOR 变异，交替选取首 512 字节或整个输入，修改 1/2/4/8
  字节。xorshift64 初始种子为 `0x8bd42f193a7e6501 XOR case_index`；
  索引先在 u64 上取模，再转为内存下标。

解析接受的 TEX 尝试首 mip 的首 surface RGBA8/RGBA32F；SKLB 尝试提取
骨架；PAP 尝试首 binding 的 spline 解码和 125 ms 采样；MTRL 查询颜色表。
其他入口检查解析和析构。正常返回格式错误或预算错误符合这种安全检查，
不能据接受计数宣称变异文件的全部语义正确。

## 资源与失败边界

每个输入最多 16 MiB，累计最多 64 MiB、64 个种子。Havok 分配限 16 MiB、
数组和值各 200000、验证工作 1000000；spline 分配限 8 MiB、tracks 4096、
blocks 1024、controls 100000、解码工作 1000000。TEX/采样输出限 1 MiB，
采样工作限 100000；其他解析器使用其默认预算。数值属于验证工具的资源
界限，不是文件格式限制。

原生 Rust 检查在 Linux x86-64 执行。`catch_unwind` 捕获到的 panic 使
检查失败，并以 create-new 保存发生问题的字节；异常进程终止、abort 或
超时同样由 Python 调用者报告失败，但不保证取得中止位置的快照。WASM
编译由普通检查覆盖；本组不冒称执行了 WASM 变异测试。

## 固定结果

总计 18123 次尝试，包含 29 次原件预检和 18094 次变异；没有 panic 或
异常进程退出。11161 次解析接受，5857 次指定后续查询/解码接受。后续
计数只描述工具所选操作，其他入口的零后续计数表示没有调用，并非失败。
TEX 后续接受计数以 RGBA8 结果记录，RGBA32F 也调用但可以返回不支持格式。

报告摘要为 `22ba6281624194eb73831f5fa3d7c2aeff845bd750479f622ee7891537b48639`，
有序种子列表摘要为
`65b1e56c10b54b0d908f3f51fceec9c0369946d3428236909197056c51d3dbb9`。
完整报告逐种子保存身份和结果计数；生成器及预算在工具源码中固定。
这属于有界的安全回归，不能证明所有非法输入都不会 panic，也不增加
生产文件变体、成功解码或格式语义的验收计数。

```sh
nix develop -c cargo build --features local --examples
python tools/verify_mutation_safety.py /path/to/game/sqpack \
  /tmp/opencode/render-regression-suite /tmp/opencode/mutation-safety
```
