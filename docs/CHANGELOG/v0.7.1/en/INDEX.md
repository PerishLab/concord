# Concord v0.7.1

Concord memory operations can now emit opt-in Locus function observations.
The environment-owned gate defaults muted and returns before bootstrap. When
enabled with an explicit file reporter, function entry and normal return share
one process-cycle trace context while every function keeps its own span
identity. A shared trace file can carry that identity across processes and
applications.

This surface deliberately covers only the memory command path. Concord owns
its readonly context seat and configuration; kernel functions only read the
seat and append atomic source facts. Invalid observation configuration is
handed to stderr without replacing the command result.

The first self-observation cycle also shortened member agreement checks. One
Git seat query now proves a member's common repository identity and branch
together, reducing the hot memory-patch audit path from four Git processes per
member to three. Resource dispatch moved into its own module when that added
observation exposed the previous structural boundary.
