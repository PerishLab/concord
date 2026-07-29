# Concord v0.5.3

## 原生 Windows task worktree

Concord 现在会把文件系统规范化产生的 `\\?\` 路径转换为 Git 能正常处理的普通
Windows 路径。task start、member add、audit、repair、landing preflight 与
remove 因此可以在原生 Windows worktree 上运行，不再被 Git 拒绝本来有效的路径。

如果 `git worktree add` 已创建分支、却在挂接 worktree 前失败，Concord 会删除
这个刚创建的分支。失败的 member add 不再留下占用 task 名称的孤立分支。

## 持续验证 Windows 交付

guard lane 新增原生 Windows runner，执行 Rust 格式检查、Clippy、测试与
PowerShell manager smoke。仓库 guard 在 Windows 上会选择原生 manager smoke，
PowerShell manager 能正确接收单个选项，release metadata smoke 也改用 Deno，
不再依赖未声明的 `jq`。

CLI 配置测试改用平台原生路径拼接。仅适用于 Unix 的 recovery fixture 也显式限制
为 Unix，使 Windows Clippy 只检查该平台实际会编译运行的代码。
