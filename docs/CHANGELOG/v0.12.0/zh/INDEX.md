# Concord v0.12.0

## 有界 Task 入口

`task brief --domain DOMAIN` 现在会投影固定大小的一页当前 Task Goal、Focus、
Question 与 Next facts。精确 cursor 让完整 Task 与 Phase 历史继续保持 opt-in，
同时为操作者提供确定性的 portfolio 入口视图。

## 可读的 Member 健康状态

`member attach` 返回规范化的已挂接 Member；`member status` 投影本地 worktree
cleanliness、Boundary currency、integration relation、已配置 upstream divergence
以及包含该 HEAD 的本地 tracking refs。Status 只读、绝不 fetch，也不声称这些
本地观察是当前 remote truth。

## 客观的 session 感知

Task-scoped command 会按永久 Task key 与 Claude、Grok 或 Codex session 各保留
一条最新私有 touch。在固定近期窗口内观察到不同 session 时，Concord 发出包含
agent、session、operation 与 time 的非阻断警告。

Activity ledger 是辅助观察，不是 Keel protocol state；它不会改变 Task revision、
authorization、lifecycle、audit agreement 或主命令结果。Concord skill 对只读操作
通常继续；写入前检查 revision、Member、Claim 与本地 Git 影响面，重叠或不明确时
向调用方复述事实和影响。
