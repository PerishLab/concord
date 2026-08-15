# Concord v0.12.5

## fact role 有了名字，Addition 是在办工作的台账

随版本发布的 skill 从未写出任何一个 fact role。agent 只能靠模仿现有 Task 学会词汇，
而 **Addition 一次都没出现过** —— 一条 Task 可以被推进数月，操作者却不知道这个 role 存在。

skill 现在写出每个 role 及其基数，并说明 Addition 是做什么的：**在办工作的台账**，
一项一条，用 title 指名、用 origin 记出处。Phase 只在 Outcome 为真时才闭合，所以正在等待、
被阻塞、或分批交付的线**没有地方 settle**，只能一路累积 Addition。settle 时把它们排空 ——
已闭合的成为 Outcome 与 Evidence，未了的成为 Phase Carry，同一封套的 edits 结束被消费掉的
Addition。Focus 保持为唯一的当前状态。

estate 什么都没变。Addition 本就带着 rank、title 与 origin，只是文字没说。

## 机器输出去掉缩进

`--json` 现在打印一行。编码后的值完全相同，而人类渲染器一直是另一条路径，
所以一次有界的 Domain 投影少掉约三分之一字节，且不少一个字段。

## 已退休的 Task 不再为一条 Phase 观察买单

退休的谱系不可能长出 legacy Phase，但每一次 mutation 之前都要走一遍所有退休 Task 的 Phase。
退休 Task 保留它的可读性 fault —— 那是唯一能拒绝写入的检查 —— 只失去 migrated-Phase 观察。
一次全域 audit 落到原先约五分之四的时间。
