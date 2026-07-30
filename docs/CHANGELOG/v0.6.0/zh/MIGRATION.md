# 迁移到 Concord v0.6.0

task protocol 与既有 task 数据不需要迁移。

CI 安装应使用 `PerishLab/actions/setup-binary@main` 并设置
`PERISH_SETUP_PRODUCT=concord`。non-stable 验证还必须给出精确 channel
版本，并始终保持隔离。
