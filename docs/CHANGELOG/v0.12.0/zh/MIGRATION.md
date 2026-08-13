# 迁移到 Concord v0.12.0

Concord v0.12.0 保留 v0.11.0 的 Keel model 与 estate protocol。现有 v0.11.0
Space 可以直接打开；不需要数据迁移、backup conversion 或 release artifact。

首次执行携带且仅携带一个已识别 session identity 的 Task-scoped command 时，
Concord 会惰性创建 `.concord/activity/<task-key>.json` 及其 lock。Activity 目录
为私有 `0700`，文件为 `0600`；每个 Task 文件在固定边界内为每个 session 保留
一条最新 touch。这些文件是辅助观察，不得作为 Task state 手工编辑。

Concord 识别 `CLAUDE_CODE_SESSION_ID`、`GROK_SESSION_ID` 与
`CODEX_THREAD_ID`。没有已识别 identity 时 observation 保持静默；出现多个 identity
或 activity 文件不可读时只会产生警告，不会替换命令结果。

成功 JSON 继续写入 stdout。结构化 session 警告写入 stderr；把任何 stderr 都
视作命令失败的 automation 应改用 process exit status，并检查 warning code
`concord.activity.concurrent_session` 或 `concord.activity.unavailable`。

安装 stable v0.12.0 manager 后运行：

```sh
concord --json audit
concord --json task show TASK
```

零 audit fault 证明 protocol agreement。Recent-session warning 仍然只是观察且
不会阻断操作。
