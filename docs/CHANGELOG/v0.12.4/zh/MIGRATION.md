# 迁移到 Concord v0.12.4

Concord v0.12.4 保留 v0.12.3 的 Keel model 与 estate protocol。现有 Space 可以
直接打开，不需要 backup conversion 或 estate migration。

今天就能发出合法 envelope 的操作者什么都不欠。被接受的输入未变，新的只是对它的
描述。拒绝保留原有的 `concord.input.json` 码与消息，并新增填充过的 `details`。

已经在解析 Atom 报告的消费方欠一处改动。`cli.finish` 的 payload 新增 `fault`
字段，携带类型化的拒绝码，成功时为 null。更早版本写下的记录没有这个字段，所以混合
报告里两种都会出现。

安装 stable v0.12.4 manager 后运行：

```sh
concord --json audit
concord phase settle --help
```

零 audit fault 证明 protocol agreement。帮助的末尾现在就是精确的 envelope。要从
拒绝里读到同一份形状：

```sh
echo '{"version":1}' | concord --json phase settle
```

错误里带着 `details.envelope`。
