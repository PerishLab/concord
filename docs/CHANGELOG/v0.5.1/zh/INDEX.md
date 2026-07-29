# Concord v0.5.1

## Agreement 闸门 mutation，hygiene 只报告

Fail-closed agreement 覆盖四个面：registry 条目、task root 下的路径存在性、
canonical 源仓库身份，以及 Git worktree 元数据。此前普通 mutation 会被**任何**
audit finding 挡住，包括 `.task/` 下的权限模式与链接。后者描述的是 Concord 自身
私有存储的卫生状况，与那四个面是否一致无关。

过宽的闸门形成了一个没有命令能打开的闭环。out-of-band 进入的 resource 树可能
含符号链接，而 `resource import` 前置拒绝链接、因此从不会创建这样的树。
`permissions normalize` 是清除权限 finding 的唯一手段，却在遇到第一个链接时中止，
且此前已对部分遍历生效。`resource remove` 是删除承载该链接的树的唯一手段，
却被它自己的删除本可清除的 finding 拒绝。这样的 seat 只能绕过 Concord 才能清理。

Audit 现在把 agreement 与 hygiene 分开报告，两者存在时仍然以非零退出。
只有 agreement 闸门普通 mutation。

## normalize 会完成它能修的部分

`permissions normalize` 遇到符号链接时跳过而非中止，并在任何动作发生前于 plan 中
逐条列出将要跳过的链接。在承载这些链接的树被移除之前，audit 会持续报告它们。
