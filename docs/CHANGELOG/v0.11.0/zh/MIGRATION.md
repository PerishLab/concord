# 迁移到 Concord v0.11.0

本次发布保留 v0.10.0 estate 格式与长期操作协议，但把一次性的 legacy transition
移出当前 binary。已经通过 v0.10.0 激活的 Space 无需数据迁移；legacy Space 必须
使用以下版本所属脚本对。

## 前置条件

先使用最新 v0.9.x binary。每个 Domain registry 都必须是 version 3，整个 Space
audit 必须 agreement，integration checkout 必须 clean，所有 worktree 与 Artifact
payload 必须留在原位。停止其他所有 Concord mutation，并保留可恢复的文件系统
backup 或只读 clone。

从精确 v0.11.0 stable seal 获取 `migration.sh` 或 `migration.ps1`。对 Plumb
而言这些 artifact 是不透明文件，但 seal 会以内容寻址方式绑定它们。每个脚本
都会独立 pin 并验证不可变 v0.10.0 manager，再把历史 engine 安装到临时隔离
seat。

脚本要求一个显式且已存在的 Space root。它会拒绝 active/partial estate、符号
链接、未知 `.concord` 条目、非法 stage payload 与含糊参数，也绝不会替换已安装
的 Concord binary。

## Survey 与 stage

Unix：

```sh
sh migration.sh --root /exact/space survey
sh migration.sh --root /exact/space stage
```

Windows：

```powershell
./migration.ps1 -Root C:\exact\space -Action survey
./migration.ps1 -Root C:\exact\space -Action stage
```

保留返回的 `census.fingerprint`。`stage` 会在
`.concord/migration/v0.10.0/stage` 下创建并验证 estate，但不会 fence legacy
binary。Unix 脚本只会 normalize 已通过验证的 Concord 迁移父目录与文件。

若已有旧 stage，不要删除或覆盖。只有 live legacy evidence 仍具有相同 fingerprint
时才能 resume；若后续工作已让它失效，应先把它保存在 active `.concord` 迁移
territory 之外，再创建 fresh stage。

Unix resume：

```sh
sh migration.sh --root /exact/space resume FINGERPRINT
```

Windows resume：

```powershell
./migration.ps1 -Root C:\exact\space -Action resume -Fingerprint FINGERPRINT
```

Fingerprint 变化必须先调查；不得改写 evidence 来强迫 agreement。

## Activation

Activation 是独立的破坏性操作。复核精确 Census，保留 backup 与 stage，并确认
所有旧 Concord process 均已停止。

Unix：

```sh
sh migration.sh --root /exact/space activate FINGERPRINT --apply
```

Windows：

```powershell
./migration.ps1 -Root C:\exact\space -Action activate -Fingerprint FINGERPRINT -Apply
```

历史 engine 会重新检查 agreement，fence 每个 legacy registry，把精确 registry
与 MAIN/PHASE evidence 归档到 v0.10.0 rollback seat，把 filesystem resource
rename 为 Artifact，再激活 staged database 与 sudo possession。随后脚本验证结果
仅包含已知 territory；Unix 会 normalize 对应受管 mode。

成功后使用当前 v0.11.0 binary 运行 `concord --json audit`。零 fault 才证明普通
mutation agreement。继续保留 rollback manifest 与 evidence；删除它们仍需要另一项
精确目标授权。
