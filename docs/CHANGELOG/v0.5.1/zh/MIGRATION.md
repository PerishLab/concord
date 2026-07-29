# 迁移到 Concord v0.5.1

已有 task、registry、worktree 与 task memory 不需要数据迁移。

权限与链接 finding 存在时 mutation 现在可以照常进行，因此此前被这类 finding 挡住的
task 无需任何修复步骤即可操作。`concord audit` 仍会报告它们，也仍以非零退出。
如果自动化把非零 audit 理解为「任何命令都不会执行」，请改为读取人类报告中的
`hygiene` 分组，或按 `kind` 对 JSON `faults` 数组分类，再逐条判断。

`permissions normalize` 不再因 `.task/` 下的符号链接失败。此前以报错结束的 plan，
现在会逐条列出跳过的链接，并对其周围的每一处模式照常生效。
