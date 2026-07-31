# Concord v0.8.0

Concord 现在分配的是仓库内的可变路径，而不只是 worktree 席位。Registry
version 2 要求每个 member 声明规范化的仓库相对 `write` 前缀。域空间全局锁
会在所有共享同一 Git common-directory 身份的活动任务之间比较 claim：精确
重合与祖先/后代重合会被拒绝，组件边界不同的兄弟前缀仍可并发。已有 claim
只能以 union 方式扩张。

交付新增了机械化边界证明。`concord member boundary` 调用稳定 Plumb library
证明已提交的 Git delta 全部位于 claim 内，并绑定 member HEAD、merge-base、
规范化 claim digest、Plumb proof schema 与实际解析到的 Plumb 版本。HEAD 变化
或 claim 扩张都会让记录失效。Version 2 的 landed removal 除原有 clean 与
reachable/tree-equivalent 检查外，还必须持有当前有效证明。

Registry version 1 仍可用于读取、审计、landing 与清理，但不能新增或扩张
member。`concord domain migrate` 只有在每个活动 member 都被显式分配 claim，
且提议状态无冲突时，才会原子升级整个域。
