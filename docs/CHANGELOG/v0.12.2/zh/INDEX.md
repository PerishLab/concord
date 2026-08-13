# Concord v0.12.2

## Claim 可以缩小；证据 Member 可以退出

`member claim` 仍然只扩张。`member narrow` 用给定集合整集替换 claim，结束已持有
的 Boundary，并拒绝 `prove` 也不会通过的拟议集合。缩小后若已提交的工作落在剩余
claim 之外，拒绝码是 `concord.boundary.outside`。没有 force 豁免。

`member release` 仍然拒绝未落地的 Member。`member retire` 是另一扇门：同样的
clean 与 current-proof 闸，不要 landed 检查，且同 Task 上至少有一个 Artifact 名
匹配 `--artifacts`。`*` 匹配任意名字，其余按字面。零命中拒绝。壳下要写成
`'*'`。

随版本发布的 Concord skill 会写下这两个动词。扩张和缩小都会作废先前的
Boundary。未落地的证据 Member 对着匹配的 Artifact 退休，而不是 reset 到 main。
