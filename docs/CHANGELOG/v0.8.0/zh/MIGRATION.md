# 迁移到 Concord v0.8.0

新增 member、扩张 claim 或记录 boundary proof 前，registry 必须升级到
version 2。先盘点域内所有活动 member，并为它们分配最小且真实的仓库相对
路径前缀：

```bash
concord domain migrate perish.code \
  --claim task-a/repo-a=crates/lib \
  --claim task-a/repo-a=docs \
  --claim task-b/repo-b=. \
  --apply
```

每个活动 member 至少出现一次；同一 member 的重复项会组成一个规范化 claim。
只有明确需要整仓所有权时才使用 `.`。命令会拒绝未知 member、缺失 claim、
非法路径，以及与共享同一 Git 身份的任一活动 member 发生的重合。

迁移后，新建 member 必须携带至少一个 `--write`。Landing 之前运行
`concord member boundary <task> <member>`；如果 HEAD 或 claim 发生变化，
必须在 landing 前重新证明。`member preflight` 与 `remove-landed` 会拒绝
version 2 下缺失或过期的 proof。

Version 1 registry 仍可读取、审计、执行 landing preflight 与清理。早于
v0.8.0 的 Concord 会拒绝 version 2 registry，不会静默忽略 claim。

Version 2 会拒绝新建 orphan member，因为当前 Plumb boundary proof 需要 commit
merge-base。既有 version 1 orphan member 仍保留审计与清理路径。
