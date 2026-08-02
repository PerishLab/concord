# Concord v0.9.2

Concord 的可选 readonly observation seat 现在会用产品自有的
`cli.start` 与 `cli.finish` Atom 事实包围每个成功解析的 CLI 命令。两条事实
共享一个 process-cycle span；finish 只在 dispatch 返回后记录命令名与退出码。

已有 kernel 函数 trace 继续作为拥有独立 span 的事实。所有记录继承同一个
trace identity，包括环境存在时精确采集的 `CODEX_THREAD_ID`。Gate 继续保持
default-muted，禁用时会在 Locus bootstrap 前返回。

Concord 不消费 observation Context，也不提供审计 query；它不推断 executor
ownership、activity、liveness 或 authority。Operator 继续通过独立的
`locus query` CLI 读取 append-only Atom stream。
