# Concord v0.7.1

Concord 的 memory 操作现在可以按需上报 Locus 函数观测。环境控制的门控默认
关闭，并在 bootstrap 之前直接返回。显式启用文件 reporter 后，函数进入与正常
返回共享同一个进程周期 trace context，每个函数则保留独立的 span identity。
共享 trace 文件可以继续把这个 identity 带过进程与应用边界。

当前观测面刻意只覆盖 memory 命令路径。Concord 管理只读 context seat 与配置，
kernel 函数只读取 seat 并追加原子 source 事实。无效观测配置会交给 stderr，
但不会替换命令自身的结果。

第一轮自观测也缩短了 member agreement 检查。一次 Git seat 查询现在同时证明
member 的公共仓库 identity 与 branch，使 memory patch 热路径上的每个 member
从四次 Git 进程降为三次。新增观测暴露出原有结构边界后，resource dispatch
也进入了独立模块。
