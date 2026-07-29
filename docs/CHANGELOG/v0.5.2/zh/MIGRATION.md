# 迁移到 Concord v0.5.2

已有 task、registry、worktree 与 task memory 不需要数据迁移。audit 的 JSON
保留了原有的每一个字段，位置不变。

task 的 `status` 不再随文件系统或主机内存压力升高，因此此前因机器原因报 `WARN`
的 task 现在报告的是它自身 footprint 的状态。以 `resources[].status` 作为闸门的
自动化会看到更少的告警，且剩下的每一条都指向一个 footprint。需要针对机器容量
行动时，请直接读取 `resources[].filesystem` 与 `resources[].host_memory`。

在同一次 audit 内，`resources[].host_memory` 现在在所有 task 之间完全一致。如果
自动化此前把各 task 的读数当作对机器的独立采样，改为只读其中一条即可。
