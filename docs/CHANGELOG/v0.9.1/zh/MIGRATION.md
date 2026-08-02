# 迁移到 Concord v0.9.1

不需要迁移 task registry 或 memory。只有显式设置
`CONCORD_LOCUS_ENABLED=true` 并配置 report file 时，observation 才会启用。

启用 observation 后，trace identity 优先级为：

1. 显式 `CONCORD_LOCUS_TRACE_ID`；
2. 精确 `CODEX_THREAD_ID` collection；
3. shared-file 或 random generation。

新建 Unix report file 使用 `0600` 权限；既有 report file 保持当前 mode。
若 operator 确实需要共享 endpoint，必须显式 provision 对应策略。

审计 query 仍位于 Concord 之外：

```sh
locus query locus.trace "$CODEX_THREAD_ID" < report.jsonl
```
