# Concord v0.6.0

本版本清偿了 Ectropy 暴露的结构债：dispatch、mutation、member seat、
task reference、path 与 checkout 操作现在都具备明确的领域 receiver 和
类型化参数束。

Concord 同时采用公共 binary 发布闭包。CLI 与 skill 在 `plumb.toml`
声明，只有 stable 可以成为默认安装共识。
