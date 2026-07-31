# 迁移到 Concord v0.7.1

无需迁移 task、registry 或 memory。除非进程继承
`CONCORD_LOCUS_ENABLED=true`，观测仍保持关闭。

启用时必须设置 `CONCORD_LOCUS_REPORT_FILE`。还可以通过
`CONCORD_LOCUS_TRACE_FILE` 共享一个生成的 trace identity，或通过
`CONCORD_LOCUS_TRACE_ID` 指定 identity。无效观测配置只产生诊断，不改变
memory 命令的结果。
