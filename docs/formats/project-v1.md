# `.vocalproj` schema v1

历史格式：当前写入 [v2](project-v2.md)，v1 经内置 migration 继续支持读取。

该格式是首个项目身份持久化版本，尚未承载完整 SDD 的音频与分析数据。

ZIP 内容（文件名区分大小写，采用 Stored，不压缩）：

```text
manifest.json
project.json
```

`manifest.json`：

```json
{
  "format": "vocal-project",
  "schemaVersion": 1,
  "createdWith": "0.1.0",
  "projectId": "cdb6a439-68bc-4c50-8689-a5adb2689c00"
}
```

`project.json`：

```json
{
  "id": "cdb6a439-68bc-4c50-8689-a5adb2689c00",
  "name": "My vocal project"
}
```

所有字段必填；未知/重复字段拒绝。UUID 按值比较，两个文件中的 ID 必须一致。
创建时使用 UUID v4；读取时接受合法 UUID 文本，写入时规范化为小写连字符格式。
`createdWith` 为非空写入器版本，不能代替 schemaVersion；每次保存记录当前写入器版本。
名称没有 trim 或 Unicode normalization，以保持用户原始文本。

每个 JSON 上限 1 MiB；容器上限 2 MiB + 64 KiB。未知包条目与压缩方式拒绝。
这些是当前最小 schema 的读取限制，音频/analysis artifact 在后续版本定义独立存储限制。
schema 的后续扩展必须更新本说明、migration 和兼容 fixtures。

加载旧版只在内存迁移；默认无历史迁移。未来版明确报错，加载失败不修改源文件。
外部程序也不得依赖私有 Rust struct 布局作为文件格式。
