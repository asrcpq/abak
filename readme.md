# Append only backup

incremental backup my append-only filesystem (aofs),
both src and dst directory can be mounted as append-only during backup.

# limitations

## abak is incomplete!

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