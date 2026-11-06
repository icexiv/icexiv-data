# 来源、许可与验证证据

## 方法与边界

解析实现由文件结构观察与验证驱动。实现会话不读取 `legacy/`；用户授权的
隔离审计只回传功能范围、位置、来源及证据等级，原实现留在审计子会话。
不引入现成 FFXIV 解析库，不复制、翻译或照搬第三方解析代码。不宣称严格 clean-room。
仓库发布状态为 `publish = false`；项目许可证需独立决定。
真实资源不提交到仓库；复现实验只读安装，派生报告与提取文件写到安装目录之外。

## 来源登记

| 来源 | 角色 | 审查与限制 |
| --- | --- | --- |
| 用户安装 `~/Games/ffxiv/game` | 原始二进制样本 | 用户授权只读与数据验证；专有资源，不再分发样本或程序 |
| xiv-companion `docs/render-book` | 已确认消费需求 | 只读取需求文档；不读取其第三方实现或解析源作为模板 |
| 本库已有 `src/`、`tests/`、`docs/` | 实现基线 | 保留原有功能及来源声明；README 所列外部来源不能代替逐项许可证与验证记录 |

## 证据记录规范

每个实验记录命令、工具版本、安装版本/平台、资源路径、提取字节数与 SHA-256、观察值、验证断言、覆盖限制。
采用公开资料结论前记录 URL、版本/commit、许可证和实际阅读范围，并以真实样本、受控实验或独立观察验证。
自身构造的回归测试用于边界与错误行为，不单独作为格式事实依据。

## 安装身份

`game/ffxivgame.ver` 文本为 `2026.10.06.0000.0000`；目录含 Win32 游戏资源。
此身份不证明跨平台布局、历史版本、每个 expansion 包或所有文件变体。

## 格式记录

- [Legacy 常用解析与历史输入的隔离对照](legacy-coverage.md)
- [IMC：许可审查、样本身份与复现](imc.md)
- [SqPack type-3：分段、对齐与模型重建](sqpack-model.md)
- [MDL：声明、几何、shape 与场景变体](mdl.md)
- [MDL 到 Win32 顶点输入的独立关联](vertex-inputs.md)
- [MTRL 与 SHPK：材质参数、颜色表和 shader 元数据](materials.md)
- [Legacy 色行转换：完整色表、双通道染色与数值对照](legacy-color-conversion.md)
- [STM：模板、列编码与 Stain 对照](stm.md)
- [TEX：多图像、浮点和 type-4 对照](tex.md)
- [Volume：完整单 mip 像素与天空采样静态证据](volume-mips.md)
- [Volume：缺口的物理存储、创建路径与像素谱系](volume-generation.md)
- [Volume：生成候选的 CPU 数值证据与目录总体](volume-generator-oracle.md)
- [天空：作者输入、base 切片与第三维采样坐标](volume-production.md)
- [天空：CPU 投影、纹理缓存与请求模式](volume-sky-consumer.md)
- [天空：投影方向、AMB 记录、实际样本与文件输出](volume-environment.md)
- [Volume：装载地址、块任务与条件性分派](volume-block-loading.md)
- [BC6H：通用 MIT 依赖、SF16 映射与独立浮点实验](bc6h.md)
- [Meddle / Physis：纹理读取路径与许可审阅](external-volume-review.md)
- [EQDP：稀疏块和位义观察](eqdp.md)
- [PBD：连接、骨矩阵与 EPBD 扩展](pbd.md)
- [PBD 标量：参数名字、湿润度与条件性顶点位置依赖](pbd-scalar-consumer.md)
- [CMP：颜色与体型范围](cmp.md)
- [EQP：压缩块、槽位区域与声明位](eqp.md)
- [SKLB/Havok：对象图与局部参考骨架](skeleton.md)
- [PAP：真实 spline 动画样本观察](animation.md)
- [AVFX / ATEX：块、曲线、几何与全部 mip 像素](vfx.md)
- [Binder：武器 IMC / AVFX / MDL / SKLB 元数据关联](binder.md)
- [SGB：家具结构指针与关联模型验证](sgb.md)
- [EXH / EXD：目录、模型和真实 subrow 字段](excel.md)

## 基础验证

`nix develop -c just check-all` 检查原生 feature 组合、回归与 WASM 编译，
覆盖 IMC、type-3/4、MDL、MTRL/SHPK、STM、TEX 多图像/浮点、EQDP/EQP、
CMP、PBD/EPBD 和 SKLB/Havok 等实现。文档构建使用
`nix develop -c notist build`，补丁空白检查使用 `git diff --check`。
真实资源验证以各格式记录列出的命令、样本哈希和比较范围为准，不由编译检查替代。

## 资源回归入口

`tools/verify_render_resources.py` 从授权安装提取固定样本，依次调用各格式的
独立观察器，覆盖 type-3、MDL、材质/shader、纹理、染色、装备表、骨架/动画、
PBD、VFX/ATEX、家具 SGB 和 Excel。输入只需 SqPack 根、固定客户端程序和
一个不存在的外部输出目录；无需手工准备仓库外的历史提取文件。

前置工具是原生 examples、Python/Pillow 和 GNU objdump。它只执行本库的
验证示例和 Python 观察器；客户端程序仅供字节身份与反汇编检查，不执行游戏。
程序、提取物、像素、反汇编及完整报告留在安装或外部输出目录，不写回安装，
不启动服务器或 GPU 测试。

```sh
nix develop -c cargo build --features local --examples
nix shell --impure --expr 'let f = builtins.getFlake "github:NixOS/nixpkgs/a799d3e3886da994fa307f817a6bc705ae538eeb"; p = import f { system = builtins.currentSystem; }; in p.python3.withPackages (x: [ x.pillow ])' \
  -c python tools/verify_render_resources.py /path/to/game/sqpack \
  /path/to/game/ffxiv_dx11.exe /tmp/opencode/render-regression
```

每一步保存命令、退出码、耗时、日志和子报告摘要；缺少/非法 JSON 子报告或
未登记样本标记使该步失败，即使验证器进程退出码为零。源文件集合、Cargo.lock、
样本清单和覆盖清单的摘要用于识别实际验证基线。失败立即停止并保留已有证据，
不覆盖输出目录。`bg.shpk` 依据固定 90101224 字节样本显式使用 128 MiB
提取预算，其余路径保持示例默认预算；不会因预算错误自动取消限制。

`report.json` 是完整机器可读记录，`summary.md` 列出回归结果与子报告链接。
`verification_suite_passed` 仅表示列出的固定回归通过，不自动将需求清单改成
完成。`known-gap-observation` 单独标记两份 volume 结构差额及其明确解码错误，
不能把正确复现错误计为这些资源解码成功。文件变体、空间语义与样本不足的
验收边界仍以[覆盖清单](../requirements-coverage.md)和各格式证据为准。

`tools/verify_volume_mips.py` 复验外部固定捕获的完整单 mip 输出，输入和命令
见 [Volume 单 mip 证据](volume-mips.md)。它逐字节比较公共 SqPack→TEX API
的实际输出与独立 zlib 解码，并同时核对完整文件读取的长度错误；不重新扫描
库存。该项报告与完整资源联跑的报告分别保存。
