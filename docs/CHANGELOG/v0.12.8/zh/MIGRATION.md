# 迁移到 Concord v0.12.8

Concord v0.12.8 保留 v0.12.7 的 Keel model 与 estate protocol。现有 Space 可以直接打开，
不需要 backup conversion 或 estate migration。

没有任何命令、封套、退出码或拒绝词汇发生变化。

**少了一条拒绝。** `concord.member.absent` 不再回答一个用自己名字寻址的任务：

```sh
concord member prove work repo --revision 3
concord member narrow work repo --claim crates --revision 4 --apply
```

两者现在与写 `perish.code/work` 完全一样地解析。已经在写全限定身份的调用方不受影响：
两种拼法解析到同一个成员，这一点对 `member status` 与 `member attach` 本来就成立。

此前靠「把任务名写全」绕开这个问题的操作者可以不再那么写，但也不必改。
**断言这条拒绝本身的脚本，应停止对一个存在的成员期待它。**

`concord.member.absent` 对「任务在但没有这个成员」和「任务不存在」仍是同一句话。
是否区分这两种缺席，仍然待决。
