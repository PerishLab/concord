# Concord v0.11.0

## 迁移退出当前产品

一次性的 legacy importer 不再属于 Concord CLI 或 `concord-core`。当前 binary
只暴露长期存在的 estate-backed Task、Phase、依赖图、Member、Artifact 与 audit
协议。

本次发布改为把 `migration.sh` 与 `migration.ps1` 作为版本所属 artifact 密封。
每个脚本都会通过 content-addressed URL 获取不可变的 v0.10.0 manager，校验固定
SHA-256 digest，把精确 v0.10.0 engine 安装到临时隔离 seat，再调用该历史 engine；
整个过程不会替换已经安装的 Concord binary。

Unix 迁移同时修复 v0.10.0 的父目录 mode 缺陷。脚本会先证明 `.concord` 仅包含
允许的迁移 territory，并拒绝链接与外来条目；只有证明成立后，才把受管目录
normalize 为 `0700`，把 estate 与 possession 文件 normalize 为 `0600`。

## Artifact 边界保持通用

Plumb 只保留并密封该版本 `artifacts/` 目录下的文件。脚本闭包、历史 engine
身份、验证、权限与 transition 语义全部由 Concord 承接。
