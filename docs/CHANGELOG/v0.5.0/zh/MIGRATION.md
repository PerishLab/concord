# 迁移到 Concord v0.5.0

已有 task、registry、worktree 与 task memory 不需要数据迁移。

如果自动化正在解析 `concord audit` 的人类可读句子，请改用全局 `--json`。人类
成功输出现在会显式命名 agreement 数据面，JSON audit 文档则新增 `resources`
数组。

当无法保留保守的本地余量时，resource import 与 member 创建现在可能在 mutation
前拒绝。请释放容量，或通过显式 task lifecycle 操作完成清理后重试；不要把这类
拒绝解释为协议不一致。

Install 与 update 默认只保留当前选中的受管版本。如果旧版本 seat 必须离线可用，
请传入 `--retain`。已发布版本仍然不可变，也仍可显式选定用于 rollback。
