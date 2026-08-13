# 迁移到 Concord v0.12.1

Concord v0.12.1 保留 v0.12.0 的 Keel model 与 estate protocol。现有 Space 可以
直接打开，不需要 backup conversion 或 estate migration。

私有 Task activity ledger 会在下一次记录 Task command 时，从 version 1 惰性升级
到 version 2；Concord 仍可读取 version-1 ledger。Version-2 touch 始终包含
`operation` 与 `time`；`agent` 和 `session` 是可缺省的 context。Ledger 仍然是
有界的辅助观察，不得作为 Task state 手工编辑。

`CLAUDE_CODE_SESSION_ID`、`GROK_SESSION_ID` 与 `CODEX_THREAD_ID` 仍然是受支持的
context 来源。仅有一个有效值时可以附加；没有值、值无效或同时存在多个值时，
context 保持缺省，但 operation 与 time 仍会保留。

安装 stable v0.12.1 manager 后运行：

```sh
concord --json audit
concord --json task show TASK
```

零 audit fault 证明 protocol agreement。Session warning 仍然只是观察且不会阻断
操作。
