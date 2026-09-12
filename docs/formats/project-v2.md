# `.vocalproj` schema v2

在 v1 的 project id/name 基础上增加关联源与轨道。ZIP 仍为 Stored 的两个文件，
每个 JSON 上限 1 MiB，容器上限 2 MiB + 64 KiB；不嵌入音频或 PCM。

manifest 与 v1 相同，但 `schemaVersion: 2`。`createdWith` 记录写入器包版本。

```json
{
  "id": "cdb6a439-68bc-4c50-8689-a5adb2689c00",
  "name": "Linked vocal",
  "sources": [{
    "id": "ddab731a-612a-49ba-8e5d-fba7d04edec2",
    "uri": {
      "kind": "linked",
      "absoluteFallback": "C:/Audio/vocal.wav",
      "relativePath": null
    },
    "contentHash": "0ff52963ef493d16ca18d47d1bd782fb7d06801f8f4d2e3847cbea3680bc86bd",
    "sizeBytes": 1004,
    "sampleRate": 48000,
    "channels": 1,
    "frames": 480
  }],
  "tracks": [{
    "id": "fa96a011-a865-48f9-ae9e-8fd8e0d04cba",
    "name": "Voice",
    "source": "ddab731a-612a-49ba-8e5d-fba7d04edec2",
    "channelMode": "equalPowerMono"
  }]
}
```

- `sources`、`tracks` 必填，允许空数组。所有实体 UUID 唯一，track.source 必须存在。
- contentHash 为完整原文件 SHA-256，64 位十六进制；写出小写。sizeBytes 必须为正。
- sampleRate/channels/frames 必须为正；frames 是每声道采样帧数（i64 Samples），不乘声道数。
- duration = frames/sampleRate，在 domain 中派生。未宣称容器元数据已经经过逐帧 PCM 解码验证。
- uri 目前只接受 linked；relativePath 可为 null/省略，若给出不允许父目录跳转或绝对路径。
  绝对 fallback 非空，使用时才检查本机路径有效性；工程可离线/跨主机打开。
- channelMode 接受 equalPowerMono/left/right/mid，表示后续通道选择意图，不修改原文件。
- 除可选 relativePath 外字段必填；当前 DTO 的未知/重复字段与未知 enum variant 拒绝。
- 嵌入音源、音符、revision、observations 等尚不在此 schema；不得通过未知字段偷偷保存。

迁移 v1→v2：严格验证 v1，然后添加空 sources/tracks，保留 id/name。源工程字节不变，
只有显式保存才写 v2。未来修改这些字段须配套 schema/migration 和兼容性测试。

Application 的相对路径解析、fallback 优先级及 Save As 重基准见 [Project-relative 切片](../architecture/phase-0-relative-source-slice.md)。这些行为复用既有 uri 字段，ProjectStore 本身不解析或核验音频。
