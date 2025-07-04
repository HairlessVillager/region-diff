## Region Diff

[English](./README.md) | 简体中文

**Region Diff** 是一款针对 Minecraft 区域文件的差分工具，支持计算、应用和压缩差分。

如果你需要定期备份游戏存档，你会发现大部分空间都被用来存储包含方块信息的 `.mca` 或 `.mcc` 文件。这些文件通常包含大量冗余内容，因为玩家更新的方块数据其实非常有限。

使用这款工具，你可以在几秒钟内计算出这些差分。由于大多数情况下，差分计算的频率远高于应用的频率，因此它能大大节省备份游戏存档的时间和空间。

目前我们支持以下文件的差分：
- `region/*.mca`
- `region/*.mcc`
- `entities/*.mca`

我们在 Minecraft Java Edition 1.21.4 上开发，因此旧版本可能会不支持。

### 快速入门

#### `diff`

假设你需要计算文件 `t1/r.0.0.mca` 和 `t2/r.0.0.mca` 之间的差分，并将差分文件保存到 `diffs/` 目录中。你可以使用以下命令：

```bash
region-diff region-mca diff t1/r.0.0.mca t2/r.0.0.mca diffs/r.0.0.mca.t1-t2.diff
```

这将计算两个文件之间的差分，并将其保存到 `diffs/r.0.0.mca.t1-t2.diff` 文件中。

#### `patch`

计算完差分后，你可以删除 `t2/r.0.0.mca` 文件，因为你可以通过以下命令重新生成它：

```bash
region-diff region-mca patch t1/r.0.0.mca diffs/r.0.0.mca.t1-t2.diff t2/r.0.0.mca
```

这会将差分作为补丁应用到旧文件 `t1/r.0.0.mca` 上，从而生成新的文件 `t2/r.0.0.mca`。

#### `revert`

你也可以只保留 `t2/r.0.0.mca` 文件，然后通过差分文件反向生成 `t1/r.0.0.mca` 文件：

```bash
region-diff region-mca revert t2/r.0.0.mca diffs/r.0.0.mca.t1-t2.diff t1/r.0.0.mca
```

#### `squash`

假设你又收到了一个新的版本 `t3/r.0.0.mca`，并且通过 `patch` 命令生成了 `diffs/r.0.0.mca.t2-t3.diff`。你可以使用以下命令将这两个差分文件合并：

```bash
region-diff region-mca squash diffs/r.0.0.mca.t1-t2.diff diffs/r.0.0.mca.t2-t3.diff diffs/r.0.0.mca.t1-t3.diff
```

你可以将生成的 `diffs/r.0.0.mca.t1-t3.diff` 文件视为两个差分文件的“合并”版本。它包含了从 `t1` 到 `t3` 版本的所有变化，你可以通过 `patch` 和 `revert` 命令来应用它。

#### 其他参数

- `-t`：并行计算的线程数，默认为 8。如果你的主机正在运行其他服务（比如游戏服务器），建议不要将这个值设置得过高。
- `-v`：程序日志的详细程度。默认情况下，程序不会显示日志。使用 `-v` 可以显示 INFO 级别的日志，使用 `-vv` 可以显示 DEBUG 级别的日志，而 `-vvv` 则会显示 DEBUG 级别的日志并将它们记录到 `debug.log` 文件中。
- `-c`：差分文件的压缩类型，默认为 Zlib。

更多详细信息，请参阅 `region-diff help`。

### 注意事项

为了提高扩展性，**Region Diff** 不会在差分文件中保存旧文件和新文件的元数据（比如文件名或哈希值）。这意味着你可以将 `diffs/r.0.0.mca.t2-t3.diff` 应用到 `t1/r.0.0.mca` 文件上，尽管这样做通常没有意义。你需要手动维护差分文件与对应的旧文件和新文件之间的关系。

同样，差分文件也不会记录自己的压缩类型，因此你需要手动记录这些信息。

### 贡献

**Region Diff** 非常依赖单元测试来确保其在不同环境下的正确性。然而，目前的测试数据还不够多样化。如果你愿意贡献自己的数据，请按照以下步骤操作：

1. **定期备份你的游戏存档**（记得按时间顺序命名）。
2. **确定一个特定的区域** 及其 x 和 z 坐标。
3. **Clone 该项目到本地**，并将该区域对应的 `.mca` 文件的时间序列添加到 `resources` 目录中。
4. **Fork 该项目**，推送你的更改，并创建一个新的 PR。

感谢你的宝贵贡献！
