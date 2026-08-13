# 迁移到 Concord v0.12.2

Concord v0.12.2 保留 v0.12.1 的 Keel model 与 estate protocol。现有 Space 可以
直接打开，不需要 backup conversion 或 estate migration。

新动词是增量的。已持有的 Member、Claim 和 Boundary proof 在有人运行
`member narrow` 或 `member retire` 之前保持不变。`member release` 仍保留
landed 闸。

安装 stable v0.12.2 manager 后运行：

```sh
concord --json audit
concord member --help
```

零 audit fault 证明 protocol agreement。帮助面上会出现 `member narrow` 与
`member retire`。要结束一个内容已经保全的未落地证据 Member：

```sh
concord member prove TASK NAME --revision REV
concord member retire TASK NAME --artifacts NAME --revision REV --apply
```
