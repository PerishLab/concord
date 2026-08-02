# Concord v0.9.1

Concord 的可选 readonly observation seat 现在可以采集精确的 ambient Codex
thread identity。存在 `CODEX_THREAD_ID` 时，CLI 会通过具名
`codex.thread` collection 将它绑定为 `locus.trace`，并在每条接受的 Atom
中保留 collection provenance。

显式 `CONCORD_LOCUS_TRACE_ID` 仍具有最高优先级；其后依次是精确 Codex
collection、shared-file 或 random trace generation。Gate 继续保持
default-muted，禁用时会在 Locus bootstrap 前返回。

Concord 现在解析 Locus 0.2.1。新建 Unix report 使用 owner-only `0600`
权限，operator 可以通过独立的 `locus query` CLI 检索 trace 事实。
Observation Context 与 report outcome 不进入 Concord protocol state、
authorization、business output、exit status 或 control flow。
