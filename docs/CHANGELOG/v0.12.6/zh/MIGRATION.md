# 迁移到 Concord v0.12.6

Concord v0.12.6 保留 v0.12.5 的 Keel model 与 estate protocol。现有 Space 可以直接打开，
不需要 backup conversion 或 estate migration。

没有任何命令、封套、输出或拒绝发生变化。操作者什么都不欠，每个现有 Task 保留自己的名字。

这次改动通过 agent 已安装的 skill 抵达。安装 stable v0.12.6 manager 后运行：

```sh
concord skill upgrade
concord skill status
```

升级后的 brief 写明了 Task 名字承载什么，并新增了「交付目标尚不足以命名」这一条 scenario。
