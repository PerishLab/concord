# Concord v0.5.0

## 资源健康进入 task audit

`concord audit` 现在把协议 agreement 与本地资源健康作为两个独立数据面报告。
每次 task audit 都会观测 task、member、memory 和 resource seat 的实际分配占用，
并给出文件系统容量、平台可用时的 inode 容量，以及主机 available memory 与
swap。

资源观测采用保守的 `OK`、`WARN`、`CRIT` 与 `UNKNOWN` 状态，但保持 advisory：
资源压力不会伪装成协议 fault，也不会阻止诊断、落地或清理。扩大占用的操作会
使用各自基于当前余量的 preflight。

Resource import 现在会检查完整源树、保留文件系统与 inode 安全余量、在 task
lock 内复检、在分配前拒绝链接与特殊文件，并把文件内容流式复制到私有 seat。
当目标文件系统已处于严重状态时，member 创建也会拒绝扩张。

## Landing proof 变得可见

`concord member preflight` 现在为每个已声明 member 输出一条证据记录，包括
canonical repository identity、预期与实际 branch、member 与 integration 的
head/tree，以及 landed removal 是由 commit reachability 还是精确 tree
equivalence 证明。

## 更安全、更可组合的操作

- `memory write` 与 `memory settle` 可以接收一个显式 stdin payload，同时拒绝
  无法区分边界的双 stdin 输入。
- 当每个 member 都有独立的 mutable branch 与 worktree seat 时，同一个
  canonical repository 可以同时服务多个 task。Concord 以 branch 而非整个
  repository 作为可变所有权单元。
- Member 创建会在 dry run 与 apply 阶段都提前拒绝已存在的目标 branch，不改变
  registry 或 worktree 集合。
- Installer 会在激活并验证所选版本后移除旧的受管版本；如果明确需要离线本地
  rollback seat，可以使用 `--retain` 保留。

随版本交付的 Concord skill 也已记录新的 audit finding、操作门控与资源导入
保证。
