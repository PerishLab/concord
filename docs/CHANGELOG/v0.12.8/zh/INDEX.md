# Concord v0.12.8

## 成员从「被寻址的那个任务」解析

`concord member narrow work repo` 报 `concord.member.absent`。成员明明已经 attach，
`concord member status work repo` 也找得到，而这条拒绝把读者指向一次从未失败过的挂载。

`Estate::member` 按存下来的任务身份 `domain/name` 匹配 Worktree。两个调用方传的是身份：
`member_status`，以及 `attach` 收尾的那次查找。**另外五个传的是请求里的原始任务字符串**，
于是 `narrow`、`prove`、`claim`、`release`、`retire` 对每一个不带域名的任务名一律拒绝。

分歧就摆在明面上。这五处每一处都在上一行已经解析过任务，而 `release.rs` 自己往下几十行
就在写 `task.identity()` —— **同一个文件里两种拼法并存**。现在十个调用点统一读身份。

`status` 与 `attach` 恰好是操作者最先用到的两个动词，所以一个成员可以被挂上、被按名字查到，
然后拒绝每一个能推进它的动词。

## 测试只演练了能工作的那一种拼法

estate 测试里每一处任务都写成 `local/work`。于是这个缺陷对一套本来覆盖了
attach、prove、narrow、claim、release、retire 的测试完全不可见。

`named` 用任务自己的名字走完 attach、prove、narrow、claim。对着本次发布撤掉修复再跑，
它在第一个动词上就失败：`member not found: work/repo` —— 那个键少了它被存进去的域名段。

## 发布调用方不再要求派发方早已不发的输入

`plumb ship binary dispatch --version v0.12.8-beta.1` 直接被拒：
`input required for 'Exact non-stable version'`。当前 Plumb 对 exact 发布从 ref 读取
channel 与 version，一个输入都不送；而两个调用方仍把 `channel` 与 `version` 声明为
`required: true`，stable 调用方还在转发一个共享 lane 会忽略的 `version`。

共享 workflow 标注「Ignored; the version is read from the ref. Stop passing it.」
已经**两个小版本**了，于是调用方早已漂移成「拒绝当前二进制能发出的每一次派发」。
没有任何一道门说过话，第一个读者是这次发布本身。
