# Concord v0.5.2

## 主机观测退出 per-task 汇总

每次 task audit 都会在 footprint 旁边带上文件系统与主机内存观测。这两者描述的是
机器——机器上每个 task 共享同一台——却被折进了每个 task 的汇总状态。一个什么都
不持有的 repo-less task 会因为主机繁忙而报 `WARN`，而可用内存一个百分点的移动会
让整个空间的 task 同时翻转。

汇总现在只跟随 task 真正拥有的 footprint：task 树、它的 member、它的 memory，
以及它的 resource seat。文件系统与主机内存保留各自的阈值、继续出现在报告里，
但作为对机器的观测，而不是对某个 task 的判决。

## 每次 sweep 只读一次主机

主机内存此前每个 task 采样一次，因此单次 `concord audit --space` 可能对它访问的
每个 task 报出不同的机器状态。现在每次 sweep 采样一次并共享。文件系统容量仍按
task 采样，因为它是从 task 路径量出来的，而 task 可能位于自己的卷上。
