# 迁移到 Concord v0.9.0

Task todo 需要 registry version 3。Version 2 域不需要重新声明 member claim：

```bash
concord domain migrate perish.code
concord domain migrate perish.code --apply
```

这次迁移只改变 registry version；现有 task、member、claim 与 boundary proof 的语义
保持不变。Version 1 域仍需通过重复的 `--claim` 为每个活动 member 提交完整 claim，
v0.9.0 会把它直接迁到 version 3。

迁移后，v0.9.0 之前的 Concord 会拒绝这个 registry，而不会静默忽略 todo 状态。
执行域迁移前，先升级所有 writer。

创建或链接未来 task：

```bash
concord task todo add perish.code/current future
```

除非带 `--dry-run`，创建会直接执行。解除关系仍是 guarded operation：

```bash
concord task todo remove perish.code/current future --apply
```

Version 3 todo 只允许同域 target。Finish source 会报告 handoff 并保留 target；若要先
清偿 target，必须显式解除所有 incoming todo，或先完成持有这些关系的 source task。
