# Concord v0.9.0

Concord task 现在可以携带一组很小的 outgoing todo，指向普通的未来 task。
`concord task todo add` 会链接同域已有 task，或在同一个加锁操作里创建 repo-less
target 并建立关系。重复 add 保持 registry 字节不变；`task todo remove` 显式解除
一条关系。

这个模型刻意小于 issue tracker 或调度器：没有状态、负责人、优先级、租约、依赖
执行，也不复制描述文本。Target 从诞生起就进入 Concord 的普通 task 生命周期，
之后可以承接任意数量的 issue、repo、release、裁决与清偿动作。

清偿 source task 时，Concord 会把仍存在的 todo 全部显示为 handoff，并保留 target。
Target 仍被 source 引用时不能 finish。Task rename 会原子重写 incoming 引用；linked
task 在关系被解除或移交前拒绝 rehome。

Registry version 3 持有这条关系，并验证 target 排序、唯一、同域存在以及禁止自指。
旧 binary 因而会 fail closed，不会在清理 target 时静默忽略 incoming relation。
