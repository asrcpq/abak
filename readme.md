# Note

make sure that no other process is writing to dst during backup:

* this is usually not a problem for backup drive

* writing to src could also cause problem, but not very likely to cause error
(need furthur investigation).

* The best choice is to manually lock src for read only before sync,
and make dst only writable from this process.