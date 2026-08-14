# Concord v0.12.4

## stdin envelope 被公开，拒绝也报出自己的名字

`task change` 与 `phase settle` 从 stdin 读一个 JSON envelope，此前对它只字未提。
帮助只列出 flag，而解码拒绝每次只报一个缺失字段，于是学会 envelope 的唯一途径就是
反复发送、逐条读拒绝。被观测到的会话正是这么做的：其中一个先查了帮助、一无所获，
然后用五次调用一次一个地收集字段名。

每个 envelope 现在在自己的类型旁边持有一份权威形状。`task change --help` 与
`phase settle --help` 直接打印它，`concord.input.json` 拒绝也在
`details.envelope` 下携带同一份形状。**一次调用回答此前需要五次才能回答的事。**
一个测试把两份公开形状分别解码回各自的类型，所以文档化的 envelope 一旦不再是合法
输入就会让 guard 变红，而不是悄悄退化成散文。

观测的 finish 事实也以 `fault` 记下拒绝码。仅凭退出码分不出「envelope 被拒」与
「revision 过期」，这让 Atom 报告无法解释自己的失败，只能回去读 agent 会话记录。

Plumb lock 推进到 0.19.0。
