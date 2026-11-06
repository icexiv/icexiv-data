# SKLB 与 Havok 骨架来源及验证

## 来源与许可

- VlcekM/MeitouClient commit `86cbb3062743210bd99ae8529f4d6c337d0f900c`，先获取
  根 LICENSE 并确认标准 GPL-3.0，再阅读 `docs/formats/havok-tagfile.md` 全文。
  仅阅读格式研究文档，没有读取 `src/Meitou.Data/Havok/HavokTagfile.cs` 或其他实现。
  文档的目标为 Kenshi/Havok 2014，包含对该目标的反编译观察及样本声明；这些不是
  FFXIV 的直接证据。采用的 v3 wire 结论均另以本次 FFXIV 样本检验，不沿用来源
  对其实现独立性的描述，也不复制其伪代码/解析代码。
- aers/FFXIVClientStructs commit `b69073ae2d789527fc1b9a78aa7aa590e0f299dc`，MIT
  许可审查见 [PBD](pbd.md)。阅读
  `Havok/Common/Base/Math/QsTransform/hkQsTransformf.cs`、
  `Havok/Common/Base/Math/Quaternion/hkQuaternionf.cs`、
  `Havok/Common/Base/Math/Vector/hkVector4f.cs` 与 `Havok/Animation/Rig/hkaSkeleton.cs`
  全文件，包括字段偏移、方法声明和相等比较；没有调用或复制其中代码。
  这些提供 QsTransform 三组四通道及 quaternion xyzw 的公开字段命名，
  不把内存指针偏移当作文件序列化偏移。
- SoulsAssetPipeline 的根许可元数据为 GPL-3.0，未读取其 Havok 实现。
  搜索遇到 SDK/来源不明的 Havok 代码仓库，未读取其源文件。

Rust 采用独立的受检字节游标、显式预算、借用实数/字节数组与 owned 元数据图。
Python observer 使用 struct 和顺序字典展开，是自行编写的独立观察工具，未引入
现成 FFXIV/Havok 解析库或执行外部解析器。未复制、翻译或照搬解析代码，不读取
legacy，不宣称严格 clean-room。GPL 文档作为审查后的格式资料，不作为依赖加入。

## 实际样本与结构观察

用户授权只读 Win32 安装 `2026.10.06.0000.0000`，七条资源路径、文件 SHA-256
及全语义行 SHA-256 固定在 `tests/fixtures/skeleton-observations.json`。
包括 c0101/c0201/c0901/c1801、w2001、m0001、d0001 的基础骨架。
容器头由原始字节观察：m0001 为 1200（46 字节头、u16 偏移），其余为 1300
（48 字节头、u32 偏移）。偏移指向同一 Havok v3 签名并允许完整解析到 end。
没有从其他容器版本直接套用偏移。

七文件共 36 个 remembered 对象、12 个 skeleton 对象、1084 条骨记录，逐个
核对名字节、parent、lockTranslation 和全部 13008 个 rest TRS 系数位。
所有骨架的父链无环且各有一个根，名字在各自骨架中唯一；quaternion 四通道的
平方长度接近 1（约 `0.99999974..1.00000016`）。数值行为与公开 QsTransform
声明相符，Rust 不借此重新归一化或抹掉 padding/W 通道。

每文件的 animation container 引用对象 3。c0201/c1801 各有三个 skeleton，
c0901 有两个；额外骨架由 mapper 引用。工具独立核对这些对象 ID，不用数组
中首个骨架代替所有引用。映射器数据保留在通用图中，没有执行 retargeting。

## 对照方法与限制

`tools/observe_tagfile.py` 独立读取 class/member 声明、presence、列式 struct 数组、
变长整数、字符串表和对象引用，直到每文件末尾的 end。`tools/verify_skeleton.py`
与 Rust `examples/validate_skeleton.rs` 全字段逐行比较，不仅比较数量/指纹；
指纹用于固定已逐项核对的结果。

实际观察覆盖 signed parent、byte 列、字符串复用、向前对象引用、多个 mapper
对象和 12/16 浮点数组。额外 tuple、三分量 vec4 数组、i64 极值、inline/backref/null
流标签具有自行编写的受控 wire 测试，不冒充游戏生成样本或运行时证据。
tagfile 其他版本和额外 SKLB 容器版本没有实样本验收；辅助区的完整语义未确认。

rest 输出是文件的局部姿态，不证明坐标到世界空间的装配、动画采样、IK、
绑定变换或 PBD 应用顺序。这些不能由 quaternion 长度测试代替。

## 复现

```sh
nix develop -c cargo build --features local --example local_sqpack --example validate_skeleton
python tools/verify_skeleton.py /path/to/game/sqpack /tmp/opencode/skeleton-check
```

工具只读安装，提取文件、独立展开行和完整报告写到不存在的新输出目录。
验证环境 Python 3.14.8；不启动游戏、附加进程、执行 GPU 或服务器。
