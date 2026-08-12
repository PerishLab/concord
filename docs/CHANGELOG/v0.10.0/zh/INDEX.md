# Concord v0.10.0

Concord 现在以每个 Space 一座 Keel SQLite estate 存储完整结构化控制面。
Domain、Repository、永久 Task 身份、当前 Task 事实、冻结 Phase、
Member/Claim/Boundary 状态与依赖闭包只有一个数据库权威。Keel bootstrap
负责创世；普通 open 只做精确 seal replay，不会隐式演化 schema。

Task 依赖是 `source depends_on target` 有向边，权重顺序为
`unknown < context < sequence < required`。它用于暴露协作问题，不承担调度，
也不阻塞普通工作。自环和环路从 day zero 起非法。冷启动即提供邻接、邻居、
度、可达、最短路径、环路、强连通分量与确定性导出。

Task 状态通过带版本、stdin-first 的 JSON change-set 写入。Settle 在同一个
Task revision 内创建一个不可变 Phase，并应用完全显式的当前状态 delta。
新 Phase 必须恰有一个非空 Outcome。Task 名称终身保留；rename/rehome 保持
永久 Task key，并修复派生 worktree 与 Artifact 路径。

Artifact 继续是私有的直接文件系统 payload，仅在 `.task/artifacts/` 下发现，
永远不成为 Keel Resource。Repo-less Task 默认不创建目录。Estate audit 会在
普通 mutation 前检查图闭包、结构化聚合、Member agreement、私有 custody 与
派生 seat 的外来 territory；依赖 observation 仍不 gate 动作。

Migration Census 只 fingerprint 发生迁移的 byte；live Member payload 继续接受
agreement 检查，但不再被无效递归 hash。

当前 skill 收敛为封闭三文件 brief：对象与动作、热点路径和克制的复杂场景。

这是一次有意为之的破坏性权威迁移。`todo`、`memory`、通用文件系统
`resource` 与 permission-normalization 命令族均已移除。激活后不再存在活跃的
`MAIN.md` 或 `PHASE-NN.md`。请严格遵循 [v0.10.0 迁移契约](MIGRATION.md)；
不存在兼容别名或双写期。
