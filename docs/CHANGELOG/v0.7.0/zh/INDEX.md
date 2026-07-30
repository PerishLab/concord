# Concord v0.7.0

Concord memory 现在具备 stdin 优先、输入有界的生命周期。所有 memory
变更都接受标准输入；显式输入文件会在操作完整成功后默认被消费；若清理失败，
错误会明确指出变更已经生效，并返回结果 revision。

本版本还引入了需显式启用的 `concord-memory:v1` envelope。Agent 可以只投影
当前所需的固定 MAIN 分块，再把编辑后的投影直接交给 `memory patch`；Concord
会保留所有未触及的源字节，并对过期 revision quick-fail。CLI 也新增了不可变
phase 的列举与读取能力。

MAIN 与 PHASE 现在有固定的行数和字节上限。Audit 会把 schema、配额、phase
命名、连续性和混合格式问题报告为 memory hygiene；保留 16 个 phase 时会给出
不影响成功状态的任务分片提示。同一 release seal 交付的 Concord skill 包含
完整 v1 grammar，并把 stdin 作为 Agent 的默认工作流。
