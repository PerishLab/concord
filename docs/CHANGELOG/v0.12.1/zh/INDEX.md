# Concord v0.12.1

## 轻量的 Task activity

现在，每次被观察到的 Task command 都会在私有 activity ledger 中保留
operation 与 time，即使无法选出 session context。仅当环境中恰好存在一个有效的
受支持 session 时，Concord 才会把对应 agent 与原生 session 作为 best-effort
context 附加上去；这些 context 不证明操作由谁发起。

Recent-session warning 仍然只是观察且不会阻断操作。只有可比较的 context 表明：
另一个 session 的 touch 晚于当前 session 上一次 touch 时，Concord 才会警告，
因此再次回到同一个 Task 不会重复已经看过的警告。

缺失、无效或同时存在多个受支持 session 环境时，Concord 会忽略 context，且不会
发出 unavailable warning。Ledger 损坏或 lock contention 仍可产生
`concord.activity.unavailable`，但不会改变主命令结果。

随版本发布的 Concord skill 会复述 operation、time 以及任何已报告 context。
只读工作通常继续；写入前重新读取当前状态，并检查相关 Member、Claim 与本地 Git
影响面。
