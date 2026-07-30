# Concord v0.6.0

This release settles the accumulated structural debt exposed by Ectropy:
dispatch, mutation, member-seat, task-reference, path, and checkout operations
now have explicit domain receivers and typed bundles.

Concord also adopts the common binary closure. Its CLI and skill are declared
in `plumb.toml`, with stable as the only default install consensus.
