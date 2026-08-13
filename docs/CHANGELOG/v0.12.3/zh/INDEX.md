# Concord v0.12.3

## 观测记下精确命令，并抵达 estate 读取

观测席的 start 与 finish 事实现在携带精确的已解析命令，而不是它的顶层分组。一次
`task show` 记的是 `task.show` 而不是 `task`；`task change`、`task brief`、
`task list` 与 `task start` 第一次可以彼此区分。

两个面共用一套词汇。已解析的命令只给自己命名一次，私有的 activity ledger 读同
一个名字，不再自持一份副本。ledger 的每一个 operation 字符串都未改变，所以记录
下来的 activity 及其警告与 v0.12.2 逐字节相同。

七处内核读取通过 Locus 追加独立的函数事实：estate inspection、world load、
current、facts、phases、graph 与 worktrees。此前一次全域 audit 在它的 start 事实
与第一条 Git 事实之间是不透明的；那一段现在可以从记录归因，而不必靠推断。未配置
`CONCORD_LOCUS_ENABLED=true` 与报告文件时观测仍然静默，且始终不能改变命令输出、
退出码或控制流。

Plumb lock 推进到 0.18.27。
