# Concord v0.5.2

## Host observations leave the per-task rollup

Every task audit carries filesystem and host-memory observations beside its
footprints. Those describe the machine, which every task on it shares, yet they
were folded into each task's rollup status. A repo-less task holding nothing
reported `WARN` because the host was busy, and a one-point move in available
memory flipped every task in the space at once.

The rollup now follows the footprints a task actually owns: the task tree, its
members, its memory, and its resource seats. Filesystem and host memory keep
their thresholds and stay in the report, as observations of the machine rather
than verdicts on a task.

## One host reading per sweep

Host memory was sampled once per task, so a single `concord audit --space`
could report a different machine state for every task it visited. It is now
sampled once per sweep and shared. Filesystem capacity stays per task, because
it is measured from the task path and a task may sit on its own volume.
