#import "_components/support/lib.notc" as support;

# 格式支持总览

可用：所列能力有实现与验证。部分：存在解码或语义缺口。资料：没有公共解析接口。
点击格式名查看参考，展开卡片查看限制。

#support.catalog(r#"""
[
  {"title":"资源与表格","items":[
    {"name":"SqPack","status":"partial","route":"sqpack/","summary":"资源定位、提取与模型重建。","limit":"Win32 index/index2；type-2/3/4，含选组读取。短 volume 无法完整重建；主 index synonym 不解析。"},
    {"name":"EXH / EXD","status":"ready","route":"excel/","summary":"表结构、普通行、subrow 与类型化字段。","limit":"EXH v3 / EXD v2。列名需外部 schema；字符串保留原始字节，不解释 SeString。"}
  ]},
  {"title":"模型与装备","items":[
    {"name":"MDL","status":"ready","route":"mdl/","summary":"几何、LOD、骨表、shape 与材质引用。","limit":"Win32 v6。部分顶点语义与非空 Glass 缺实样本验证；不烘焙 shape 或蒙皮。"},
    {"name":"IMC","status":"ready","route":"imc/","summary":"模型 variant、部件及材质/VFX 字段。","limit":"保留原始动画字节与部件掩码；不执行动画或候选路径回退。"},
    {"name":"EQDP","status":"partial","route":"eqdp/","summary":"race/sex 的模型与材质可用位。","limit":"装备/配饰五槽稀疏表。完整材质回退语义缺证据。"},
    {"name":"EQP","status":"partial","route":"eqp/","summary":"装备显示参数与耳、尾、头发声明。","limit":"提供槽位来源和耦合字段；不计算完整组合显隐。"},
    {"name":"SGB","status":"ready","route":"sgb/","summary":"家具实例、TRS 与模型/场景依赖。","limit":"限单 scene/group 家具布局；其他实例类型仅保留公共头。"}
  ]},
  {"title":"材质与纹理","items":[
    {"name":"MTRL","status":"ready","route":"mtrl/","summary":"材质参数、色表转换与双通道染色。","limit":"支持 Legacy 16 行与 Dawntrail 32 行。实际 544 字节 Legacy dye 材质缺样本验证；不执行 shader。"},
    {"name":"SHPK","status":"ready","route":"shpk/","summary":"Shader、资源绑定、keys 与 pass 元数据。","limit":"DX11 0d01/0e01。Hull/Domain 缺非空实样本；不执行 shader 或选择 permutation。"},
    {"name":"STM","status":"ready","route":"stm/","summary":"Legacy/Dawntrail 染色模板与 stain 查询。","limit":"提供 half/f32 输出；额外历史索引表示缺输入证据。"},
    {"name":"TEX","status":"partial","route":"tex/","summary":"2D、array、cube、volume 的 mip 解码。","limit":"整数、BC1–7 支持 RGBA8；七种非压缩浮点及 BC6H 支持非截断 RGBA32F。短 volume 可读完整 base mip，但不能补齐整文件；不支持 tiled/swizzled。"},
    {"name":"ATEX","status":"ready","route":"atex/","summary":"复用 TEX 的纹理解析与像素解码。","limit":"实样本含 cube/mip；float/array/volume 缺 ATEX 实样本验证。"},
    {"name":"AMB","status":"reference","route":"amb/","summary":"环境系数、相位与目录的格式研究。","limit":"有格式参考和静态消费证据，没有公共解析接口。"}
  ]},
  {"title":"骨架与动画","items":[
    {"name":"SKLB / Havok","status":"ready","route":"sklb/","summary":"骨名、父索引与本地 rest TRS。","limit":"SKLB 1200/1300、tagfile v3。其他版本不支持；辅助区语义不完整。"},
    {"name":"PBD / EPBD","status":"partial","route":"pbd/","summary":"种族关系、骨矩阵、祖先路径与仿射数学。","limit":"矩阵可读，不等于完整变形规则已确认；source/target/bind 空间与 EPBD 选择规则缺证据。"},
    {"name":"CMP","status":"ready","route":"cmp/","summary":"外观色区、体型范围与百分比插值。","limit":"限八种族 human.cmp 布局；不计算姿态或 NPC 回退。"},
    {"name":"PAP","status":"partial","route":"pap/","summary":"动画 binding、轨道映射与 spline 采样。","limit":"Win32 0x00020001，transform spline 毫秒采样；不支持 float tracks，不组合 blendHint。"}
  ]},
  {"title":"特效资产","items":[
    {"name":"AVFX","status":"ready","route":"avfx/","summary":"节点、曲线、纹理、内嵌几何与 Binder。","limit":"限六类节点及已验证的 31 种 Data 组合；不模拟特效或合成宿主姿态。"}
  ]}
]
"""#)

验证基准：Win32 `2026.10.06.0000.0000`。支持以卡片所列能力为准，
不穷尽历史变体。GPU 渲染、角色装配与 VFX 模拟不在本次范围内。

[详细覆盖与缺口](requirements-coverage.md) · [来源与验证](provenance/README.md)
