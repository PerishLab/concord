# 迁移到 Concord v0.7.0

现有 task 与 legacy Markdown memory 不需要自动数据迁移。Legacy MAIN 在新
配额内仍可读取和整文件替换，但 section 投影与局部 patch 需要显式切换到
`concord-memory:v1` envelope。普通 write 与 settle 不会把 v1 memory 降级，
structured MAIN 与所有保留 phase 必须采用同一种格式。

Memory 变更命令现在会在完整成功后默认消费显式 `--file` 输入。需要保留源文件
时传入 `--keep-file`；settle 使用 `--keep-files`。生成内容通常应使用
`--file -` 和 stdin。`memory.cleanup_after_apply` 表示 memory 变更已经成功、
但输入清理失败；重试前应先核对错误返回的结果 revision。

MAIN 上限为 400 行、64 KiB；每个 PHASE 上限为 800 行、128 KiB；raw MAIN
读取另有 4 MiB 的诊断上限。超限的既有 memory 应压缩，或拆分为更小的后续
task。相关 audit finding 仍属于可修正的 memory hygiene，不构成 protocol
disagreement。
