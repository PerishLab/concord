# 迁移到 Concord v0.5.3

已有 task、registry、worktree、task memory、配置与受管 skill seat 都不需要数据
迁移。

Windows 操作者应同时更新 binary 与受管 Concord skill。此前因 Git 收到 `\\?\`
路径而失败的 task 操作，更新后可以重试。如果旧版本的 `member add` 失败后留下了
与 task 同名的分支，请先检查其中内容，再明确删除或保留；Concord 不会推断旧版本
所创建分支的所有权。

Linux 与 macOS 行为不变。
