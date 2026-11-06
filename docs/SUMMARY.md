# IceXIV Data

[Home](README.md)

- [格式支持总览](support.md)

# SqPack

- [SqPack](sqpack/README.md)
  - [index、index2 与 DAT 格式](sqpack/container-formats.md)

# Excel

- [Excel 表格资源](excel/README.md)

# Texture

- [TEX 纹理](tex/README.md)
- [AMB 环境系数](amb/README.md)

# Model variants

- [MDL 模型](mdl/README.md)
- [IMC 模型变体](imc/README.md)
- [EQDP 装备变形参数](eqdp/README.md)
- [EQP 装备显示参数](eqp/README.md)

# Materials and shaders

- [MTRL 材质](mtrl/README.md)
- [SHPK Shader Package](shpk/README.md)
- [STM 染色模板](stm/README.md)

# Skeleton and deformation

- [SKLB 骨架容器](sklb/README.md)
- [Havok binary tagfile v3](havok/README.md)
- [PAP 动画包](pap/README.md)
- [AVFX 特效资产](avfx/README.md)
- [ATEX 特效纹理](atex/README.md)
- [SGB 共享场景与家具依赖](sgb/README.md)
- [CMP 角色外观参数](cmp/README.md)
- [PBD 骨骼变形](pbd/README.md)

---

# Validation

- [渲染需求覆盖](requirements-coverage.md)
- @(collapsed: true)[来源与证据](provenance/README.md)
  - [Legacy 解析能力对照](provenance/legacy-coverage.md)
  - [实资源变异安全验证](provenance/input-safety.md)
  - [IMC 证据](provenance/imc.md)
  - [SqPack 模型重建证据](provenance/sqpack-model.md)
  - [index2 碰撞名称与位置对照](provenance/sqpack-index2.md)
  - [MDL 证据](provenance/mdl.md)
  - [Win32 顶点输入证据](provenance/vertex-inputs.md)
  - [MTRL 与 SHPK 证据](provenance/materials.md)
  - [Legacy 色行转换证据](provenance/legacy-color-conversion.md)
  - [STM 证据](provenance/stm.md)
  - [TEX 证据](provenance/tex.md)
  - [Volume 单 mip 与天空采样证据](provenance/volume-mips.md)
  - [Volume 缺口的存储与生成证据](provenance/volume-generation.md)
  - [Volume 生成候选的 CPU 数值证据](provenance/volume-generator-oracle.md)
  - [天空作者输入与采样坐标证据](provenance/volume-production.md)
  - [天空 CPU 投影与纹理缓存证据](provenance/volume-sky-consumer.md)
  - [天空方向与 AMB 系数证据](provenance/volume-environment.md)
  - [Volume 装载地址与块任务证据](provenance/volume-block-loading.md)
  - [BC6H 依赖与独立浮点验证](provenance/bc6h.md)
  - [Meddle / Physis 审阅](provenance/external-volume-review.md)
  - [EQDP 证据](provenance/eqdp.md)
  - [PBD 证据](provenance/pbd.md)
  - [PBD 标量的 shader 消费证据](provenance/pbd-scalar-consumer.md)
  - [CMP 证据](provenance/cmp.md)
  - [EQP 证据](provenance/eqp.md)
  - [SKLB 与 Havok 骨架证据](provenance/skeleton.md)
  - [PAP 样本观察](provenance/animation.md)
  - [AVFX / ATEX 证据](provenance/vfx.md)
  - [Binder 武器宿主元数据证据](provenance/binder.md)
  - [SGB 家具依赖证据](provenance/sgb.md)
  - [EXH / EXD 实际表证据](provenance/excel.md)

---

- [Notist components](_components/README.not)
