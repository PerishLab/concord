# 迁移到 Concord v0.12.3

Concord v0.12.3 保留 v0.12.2 的 Keel model 与 estate protocol。现有 Space 可以
直接打开，不需要 backup conversion 或 estate migration。

让观测保持静默的操作者什么都不欠 —— 静默是默认状态。Task activity 记录及其警告
未改变。

已经在解析 Atom 报告的消费方欠一处改动。`cli.start` 与 `cli.finish` 的 payload
`command` 字段此前携带 `task`、`member` 这样的顶层分组，现在携带 `task.show`、
`member.attach` 这样的精确命令。按字面匹配过旧值的读取方必须改匹配新值。更早版本
写下的记录保留旧值，所以混合报告里两种都会出现。

安装 stable v0.12.3 manager 后运行：

```sh
concord --json audit
concord --version
```

零 audit fault 证明 protocol agreement。要确认更细的命令事实：

```sh
CONCORD_LOCUS_ENABLED=true CONCORD_LOCUS_REPORT_FILE=/tmp/atoms.jsonl \
  concord --json task list
locus query locus.trace < /tmp/atoms.jsonl
```

报告中的 `cli.start` payload 记的是 `task.list`。
