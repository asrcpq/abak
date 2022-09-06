# Append only backup

incremental backup my append-only filesystem (aofs),
both src and dst directory can be mounted as append-only during backup.

# Notes

## abak is incomplete!

It only works for obvious cases of moved and appended files.

For example if a file matches multiple possible appended versions in src directory,
abak will simply quit.

Since abak will construct the operations first before perform final execute,
fail in middle will not cause any data change.

## prevent data change during write

make sure that no other process is writing to dst during backup:

* this is usually not a problem for backup drive

* writing to src could also cause problem, but not very likely to cause error
(need furthur investigation).

* The best choice is to manually lock src for read only before sync,
and make dst only writable from this process.