## Region Diff

English | [简体中文](./README_zh.md)

**Region Diff** is a differential tool for Minecraft Region Files, supporting the calculation, application, and squashing of differences.

When you want to periodically back up your game saves, you end up using 95% of the space to store large `.mca` or `.mcc` files, which contain block information. An obvious conclusion is that most of their content is redundant because players only update a very limited amount of block data. 

Using this tool, you can calculate the above differences in just a few seconds. Considering that most scenarios involve more differential calculations than applications, this will save a lot of time in backing up game saves while saving a lot of space!

We currently support the following file differentials:
- `region/*.mca`
- `region/*.mcc`
- `entities/*.mca`

We are developing on Minecraft Java Edition 1.21.4, so older versions may not be supported.

### Quickstart

#### `diff`

Suppose you need to calculate the difference between the files `t1/r.0.0.mca` and `t2/r.0.0.mca`, and you want to store the difference file in the `diffs/` directory. You can use the following command:

```bash
region-diff region-mca diff t1/r.0.0.mca t2/r.0.0.mca diffs/r.0.0.mca.t1-t2.diff
```

This will calculate the difference between the two files and save it to `diffs/r.0.0.mca.t1-t2.diff`.

#### `patch`

You can now delete the `t2/r.0.0.mca` file, as you can recreate it using the following command:

```bash
region-diff region-mca patch t1/r.0.0.mca diffs/r.0.0.mca.t1-t2.diff t2/r.0.0.mca
```

This will apply the difference as a patch to the old file `t1/r.0.0.mca` to obtain the new file `t2/r.0.0.mca`.

#### `revert`

Alternatively, you can keep only the `t2/r.0.0.mca` file and calculate `t1/r.0.0.mca` using the difference:

```bash
region-diff region-mca revert t2/r.0.0.mca diffs/r.0.0.mca.t1-t2.diff t1/r.0.0.mca
```

#### `squash`

Suppose a new version `t3/r.0.0.mca` arrives and you obtain `diffs/r.0.0.mca.t2-t3.diff` using the `patch` command. You can squash these two diffs using the following command:

```bash
region-diff region-mca squash diffs/r.0.0.mca.t1-t2.diff diffs/r.0.0.mca.t2-t3.diff diffs/r.0.0.mca.t1-t3.diff
```

You can think of the resulting `diffs/r.0.0.mca.t1-t3.diff` as the "merge" of the two diffs. It contains the differences between the `t1` and `t3` versions of the file, and you can apply it using the `patch` and `revert` commands.

#### Other Parameters

- `-t`: Number of threads, default is 8. When the host is running other services (e.g., a game server), it is recommended not to set this value too high.
- `-v`: Verbosity of program logs. By default, no logs are displayed. `-v` shows INFO-level logs, `-vv` shows DEBUG-level logs, and `-vvv` shows DEBUG-level logs and logs them to `debug.log`.
- `-c`: Compression type for the diff file, default is Zlib.

For more infomation, see `region-diff help`.

### Notes

For better extensibility, **Region Diff** does not maintain metadata of the old and new files in the difference file (e.g., file names or hashes). This means you can apply `diffs/r.0.0.mca.t2-t3.diff` to `t1/r.0.0.mca`, although it is mostly meaningless. You need to manually maintain the relationship between the difference files and the corresponding old and new file pairs.

Similarly, the difference file does not record its own compression type, so you need to manually maintain this information.

### Contributing

**Region Diff** relies heavily on unit tests to ensure its correct functionality across various environments. However, the current test data lacks diversity. If you're willing to contribute your data, please follow the steps below:

1. **Periodically back up your game saves** (remember to name them according to the time).
2. **Identify a specific region** and its x and z coordinates.
3. **Clone this project** and add the time series of the corresponding `.mca` file for that region to the `resources` directory.
4. **Fork this project**, push your changes, and create a new PR.

Thank you for your valuable contribution!
