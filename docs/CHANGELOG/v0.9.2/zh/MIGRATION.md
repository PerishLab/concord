# 迁移到 Concord v0.9.2

不需要迁移 task registry 或 memory。只有显式设置
`CONCORD_LOCUS_ENABLED=true` 并配置 report file 时，observation 才会启用。

启用后，每个成功解析的 Concord 命令都会产生 process-cycle 记录，不再只包含
`memory` 命令的函数记录。Operator 应按更广的覆盖范围复核 report capacity 与
retention。JSONL Atom 格式、trace precedence、endpoint 配置与 owner-only
创建权限均未改变。

`cli.finish` 是 dispatch 返回后的不可变事实，不是 liveness 或 active executor
信号。Query 仍位于 Concord 之外：

```sh
locus query locus.trace "$CODEX_THREAD_ID" < report.jsonl
```
