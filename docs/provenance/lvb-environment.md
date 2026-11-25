# LVB 环境引用的偏移基准

Windows 数据档案 `2026.11.03.0000.0000`，SqPack entry
`bg/ffxiv/est_e1/hou/e1h1/level/e1h1.lvb`。解包长度 1,086 字节，`LVB1`
头记录一个 `SCN1`。以下均为文件绝对位置和小端值。

- `SCN1` 数据基准是 `0x14`；General 相对偏移字段位于同处，值
  `0x40`，因此 General 始于 `0x54`。
- General `+8` 的环境列表相对偏移为 `0x60`；`+12` 的计数为 1。
  列表首条在 `0xb4`，四个 u32 值为 `329, 0, 6864785, 399`。
- 以 `0xb4` 为记录基准，329 与 399 分别指向 509 和 579。两处
  NUL 结尾串分别为 `bgcommon/env/global/ffxiv_genv/genv_est_e1/genv_e1_hou/genv_e1h1.envb`
  和 `bgcommon/env/sound/est_e3/e3_fld/sound_e3f1_coast.essb`。

以上是布局引用，不是客户端活动区域或天气权重。库的回归夹具由人工
构造，验证相对基准、两个资源类型的分离和坏偏移拒绝。
