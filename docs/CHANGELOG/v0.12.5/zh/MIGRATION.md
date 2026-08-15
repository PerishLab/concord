# 迁移到 Concord v0.12.5

Concord v0.12.5 保留 v0.12.4 的 Keel model 与 estate protocol。现有 Space 可以直接打开，
不需要 backup conversion 或 estate migration。没有新增、移除或改变任何 role。

操作者什么都不欠。每个 Task 的 fact 一字不动，今天在 Focus 里带着台账的线，
在有人搬动它之前会继续带着。

在终端里读 `--json` 的消费方会看到一行，而不再是缩进块。解码后的值未变，
所以解析器不需要改，需要改的只是眼睛。手工阅读时接一个格式化器：

```sh
concord --json task brief --domain DOMAIN | python3 -m json.tool
```

统计全域 `phase.missing_outcome` 观察数的消费方会看到更少的条目。该观察现在只对
活跃 Task 报出。它从未进入 audit agreement，所以没有任何拒绝行为改变。

安装 stable v0.12.5 manager 后运行：

```sh
concord --json audit
concord skill status
```

零 audit fault 证明 protocol agreement。升级后的 skill 写出了 role 词汇与 Addition 台账。
