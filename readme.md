# Dummy append only backup

incremental backup my append-only filesystem (aofs),
both src and dst directory can be mounted as append-only during backup.

# usage

```
abak <src> <dst> [--check check_ratio]
```

# steps

* exclude all files that match both size and path exactly in src and dst,
compare full contents for a proportion of these files.

* match all rest files in dst against src for first 4KB data,
all file in dst must match exactly one unique file in src.

* build the plan to move files, the rest files in src will be created

* execute the plan

# limitations

## incompleteness

It only works for obvious cases of moved and appended files.

For example if a file matches multiple possible appended versions in src directory,
abak will simply quit.

Since abak will construct the operations first before perform final execute,
fail in middle will not cause any data change.

## prevent data change during write

Currently no other process should be writing to src/dst during backup:

* usually not a problem for backup drive

* current solution is to manually lock src for read only before sync,
and make dst only writable from this process.
