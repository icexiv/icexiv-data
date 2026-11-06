#import "../_components/pele/lib.notc" as pele;

# ATEX 特效纹理

ATEX 是 AVFX 引用的纹理资源扩展名。Win32 样本使用 [TEX 的纹理头、类型与像素
格式](../tex/README.md)：头长 80 字节，包含 attributes、format、宽高深、mip
信息、array size、LOD 和 surface offsets。扩展名没有另加一个封装头。

#pele.diagram(r#"""
flowchart LR
    avfx[AVFX Tex block] --> path[NUL-terminated ATEX resource path]
    path --> resource[SqPack resource entry]
    resource --> header[TEX-style 80-byte header]
    header --> mips[Per-mip images / pixel blocks]
"""#)

实际 AVFX 路径例如 `vfx/cut/aktkma/aktkma10910/texture/bom_smk_msk.atex`。
SqPack entry 类型与纹理像素类型是两个维度；该资源为 type-2 标准文件条目，
提取后依然是 TEX 风格的完整纹理。不能以“是纹理”为由只搜索 type-4 条目。

surface offsets 以字节为单位，相对于提取后的纹理文件起点，头内数字为小端。
各 mip 按头声明的维度确定图像数量；cube 的六面、array 的多层、volume 的
深度切片都不能折叠成一张 base image。浮点格式的负值、HDR 或非有限值与
RGBA8 显示转换后的值应区分，相关字段布局和格式 ID 与 TEX 相同。

实际 ATEX 包括 2D BC1/BC3 和六面 BC1 cube，含完整的 8/9 级 mip 链，最小级
仍按 BC 块大小占用存储。`cube004_o.atex` 的 attributes 为 `0x02000000`，
128×128、八级 mip，每级包含六面，共 48 张图像，不能只保留第零面。
array/volume/浮点类型属于公共纹理结构的类型维度；实际 ATEX 样本的覆盖范围
见独立来源记录，不由扩展名推定。
