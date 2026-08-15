# 迁移到 Concord v0.12.7

Concord v0.12.7 保留 v0.12.6 的 Keel model 与 estate protocol。现有 Space 可以直接打开，
不需要 backup conversion 或 estate migration。

没有任何命令、封套或输出发生变化。每一条拒绝都保留自己的 code、message 与退出码。

三条此前带 `"details": null` 的拒绝现在带一个对象。**断言过该字段为 null 的读取方必须停止断言**：

- `concord.task.absent` 带 `identity`，再带 `tasks` 加 `active`/`retired` 计数，
  域未知时改带 `domains`。
- `concord.task.ambiguous` 带 `identity` 与相撞的 `tasks`。
- `concord.domain.absent` 带 `domain` 与受管的 `domains`。

`tasks` 最多六十四条活跃身份。`active` 大于列表长度即表示上限截断过；
`retired` 说明还有多少名字可解析但未列出。

拒绝词汇通过 agent 已安装的 skill 抵达。安装 stable v0.12.7 manager 后运行：

```sh
concord skill upgrade
concord skill status
```

在设置了 `CONCORD_LOCUS_ENABLED` 的情况下跑 Concord 自己的测试套，不会再往配置的报告文件
追加 fixture 记录。**持有本次发布之前读数的操作者，应把任何跑过 `cargo test` 的窗口视为受污染，
并重截一份基线。**
