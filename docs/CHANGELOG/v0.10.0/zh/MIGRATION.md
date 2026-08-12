# 迁移到 Concord v0.10.0

本次发布以一座 Space-wide Keel estate 替换全部历史 Domain registry 与文件型
task memory。迁移必须作为一次显式 staged transition 完成；不要让新旧 Concord
并发执行 mutation。

## 前置条件

先使用最新 v0.9.x binary。每个 Domain registry 都必须已经是 version 3，整个
Space audit 必须 agreement，integration checkout 必须 clean，既有 worktree 与
Artifact payload 必须留在原位。Survey 前停止其他 Concord mutation，并保留
Space 的可恢复文件系统备份或只读副本。

v0.10.0 migrator 会拒绝 disagreement、无法建模的 Task territory、非法 Member
claim、内容或权限漂移、悬空 todo endpoint、自依赖与环路；它不会猜测或修复。

## Survey 与 stage

让新 binary 指向精确的 Space root：

```sh
concord --json migration survey
concord --json migration stage
```

`survey` 对 registry、MAIN/PHASE、权限、Artifact payload 与 worktree agreement
建立有界 Census。Member 内容保持 live 且不参与迁移，因此不进入 Census byte
fingerprint。`stage` 会在 Space lock 内重复可迁移证据，bootstrap
`.concord/migration/v0.10.0/stage`，导入完整 estate，并验证 scalar、relation、
closure、文件系统与权限相等。它不会激活 estate，也不会 fence 旧 binary。

保留返回的 `census.fingerprint`。后续 session 只有在 live legacy evidence 仍
完全一致时才能重开同一 stage：

```sh
concord --json migration resume FINGERPRINT
```

Fingerprint 发生变化时必须先调查；不要通过删除或改写 stage 强迫 agreement。

## Activation

Activation 是独立的破坏性授权。复核 staged Census 并确认所有旧 Concord
process 已停止后，执行：

```sh
concord --json migration activate FINGERPRINT --apply
```

Activation 会在 Space lock 内重新 audit worktree agreement，并重新 hash 可迁移
legacy evidence。它首先把所有已知 registry 替换为不受旧 binary 支持的
version-4 marker，随后把精确 registry 与
MAIN/PHASE 归档到 `.concord/migration/v0.10.0/rollback`，在同一文件系统上把
`.task/resources` rename 为 `.task/artifacts`，修剪新产生的空 repo-less root，
最后把 staged database 与 sudo possession 安装为 `.concord/estate.sqlite3` 和
`.concord/sudo`。

Activation journal 会回滚未完成的激活。成功激活后有意不提供普通 rollback
命令，也不再存在可用的 legacy authority。必须保留 rollback manifest 与证据；
删除它们需要另一项精确目标授权。

## 导入语义

- 每个 legacy Domain、Repository、Task、Member、Claim 与 Boundary 保留其精确
  已证明语义；每个当前 Task 坐标获得终身 Reservation。
- 由旧 Plumb 证明的 Boundary 保持精确，并在 activation 后以不 gate 的
  `boundary.stale` observation 暴露。Member release 前重新 prove；版本陈旧不
  拒绝迁移。
- 每条 todo 以原 endpoint 导入为一条 `unknown / legacy-todo` 直接依赖。Keel
  重建 closure，任何环路都会让 stage 失败。
- 结构化 MAIN role 成为当前 fact。Collection Markdown 保持一个完整有序 body，
  不会把 bullet 拆成臆造的 Resource。
- 每个 PHASE 成为冻结 aggregate。Legacy Phase 可以没有 Outcome，并以不 gate
  的 `phase.missing_outcome` observation 暴露。
- Legacy 或未知 TOML/text 内容成为带归因的 Addition；空 section 不生成文本。
- Artifact byte 与 Git worktree 永不进入 SQLite，也不会被批量复制。

## 新操作路径

使用 `concord <command> --help` 阅读精确 grammar。主要替换关系如下：

```text
todo add/set/remove       -> task dependency add/set/remove
memory read/patch         -> task show / task change
memory settle/phase       -> phase settle / phase list
resource import/remove    -> artifact import/remove
member add/boundary       -> member attach/prove
member remove-landed      -> member release --apply
domain init               -> domain bootstrap（仅 fresh Space）
```

Task change 与 settle 默认从 stdin 读取 version-1 JSON，并携带预期 Task revision。
Dependency 写入携带预期 Space graph revision。破坏性的 remove、release、finish
与 activate 都要求 `--apply`。

Activation 后运行 `concord --json audit`，同时查看 `faults` 和 `observations`。
零 fault 才证明普通 mutation agreement。Unknown/cross-domain dependency 与缺失
Goal/Focus/Next 的事实用于暴露待整理问题；它们不授权清理，也不阻塞执行。
