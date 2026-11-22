---
name: ponytail
description: Smallest complete change. Use on any code task (write, fix, refactor, review, add dependency). Adapted from DietrichGebert/ponytail (MIT).
---

# ponytail

Solve the whole problem with the least new code. End with one line: what was skipped or unchecked, and any risk.

## Scope first
Read the task and the code it touches. List every place the change must reach: callers, tests, fixtures, config, exports. Check what it could break. That is scope. Extra features are not.

## Pick the first option that fully works
1. Needn't exist? Skip, name it in one line.
2. Already in the codebase? Reuse it the way surrounding code does.
3. Standard library or platform feature? Use it.
4. Installed dependency? Use it. Never add one for a few lines.
5. One readable line? Use it.
6. Else minimum code that works.

## Rules
- Lazy about the solution, never about completeness: update every caller, test, fixture the change breaks.
- No unrequested abstraction, wrapper, option, config, "for later" code. Deletion beats addition.
- Bug fix: grep every caller of the function once, fix the root cause in shared code.
- Moved or merged code keeps its error handling and validation.
- New non-trivial logic leaves one small test or assert.
- Known-limit shortcut gets a one-line comment: `shortcut: <limit>, <when to upgrade>`.
- Never cut: trust-boundary validation, data-loss error handling, security, anything the user asked for.

## j2zig overrides
- No stubs, no fake success, no TODO. Unsupported behavior fails loudly and appears in `report.json`.
- Fix the generator (`j2zig/`), never generated output.
