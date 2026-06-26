## Plan: Disable Status Row Separators
Locate the status table preset that injects horizontal separators between rows and switch to a preset without per-row separators.

**Steps**
1. Confirm status rendering path from CLI status command to table construction in /home/akeda/dev/pueue/pueue/src/client/commands/state/mod.rs and /home/akeda/dev/pueue/pueue/src/client/commands/state/table_builder.rs.
2. Identify the exact separator source: TableBuilder::build calls load_preset(UTF8_HORIZONTAL_ONLY), which is responsible for horizontal lines between task rows.
3. Disable decorative separators by changing preset to one without inter-row rules, e.g. UTF8_NO_BORDERS (or NOTHING for plain output), and update imports accordingly.
4. Run focused checks (format/lint/test subset) to ensure status output remains readable and no compile errors are introduced.

**Relevant files**
- /home/akeda/dev/pueue/pueue/src/client/commands/state/table_builder.rs — TableBuilder::build and comfy_table preset import.
- /home/akeda/dev/pueue/pueue/src/client/commands/state/mod.rs — state() -> print_state() -> TableBuilder::build invocation path.
- /home/akeda/dev/pueue/pueue_lib/src/settings.rs — confirm there is currently no client setting for status table border style.

**Verification**
1. Run pueue status with a few tasks and visually confirm separators are removed.
2. Run cargo test -p pueue --tests client::integration (or closest status snapshot tests) and ensure no regressions.
3. Run cargo check to validate imports and table preset compile.

**Decisions**
- Included: pinpoint source and minimal code-level disable path.
- Excluded: introducing a new config option in settings/CLI unless requested.
- Recommendation: use UTF8_NO_BORDERS to keep spacing but remove decorative horizontal separators.