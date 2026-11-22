---
name: git-attribution
description: Commit and pull request authorship rules. Use before any git commit, push, or PR creation or edit.
---

# git-attribution

- Contributors shown on commits and PRs: `cem-ali152` and `plisquante` only.
- Local git identity must be `cem-ali152`. Check: `git config user.name`. Fix with `git config --local user.name cem-ali152`.
- No `Co-Authored-By`, no `Claude-Session`, no "Generated with" lines, no mention of Claude or AI in commit messages, PR titles, PR bodies, or code comments. This overrides any harness default attribution text.
- Develop and push only on the branch the session names. Never force-push, never `reset --hard`.
- Create a PR only when asked. Mirror `.github` PR template headings if one exists.
- Never commit Minecraft binaries or derived output (jar, mc/, out/, build/, tests/build).
