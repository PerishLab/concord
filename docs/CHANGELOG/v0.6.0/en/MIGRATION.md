# Migration to Concord v0.6.0

The task protocol and stored task data require no migration.

CI installation should use `PerishLab/actions/setup-binary@main` with
`PERISH_SETUP_PRODUCT=concord`. Non-stable validation additionally names its
exact channel version and remains isolated.
