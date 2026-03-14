---
name: organize-commits
description: Organize uncommitted changes into logical, atomic commits using conventional commit format. Analyzes staged and unstaged diffs, groups related changes, excludes sensitive/useless files, and creates clean commit history.
user_invocable: true
---

# Atomic Commit Organizer

Analyze all uncommitted changes (staged + unstaged) and organize them into logical, atomic commits with conventional commit messages.

## Procedure

### 1. Gather state

Run these commands in parallel:
- `git status` (never use `-uall`)
- `git diff` (unstaged changes)
- `git diff --cached` (staged changes)
- `git log --oneline -10` (recent style reference)
- `git diff HEAD` (combined view)

### 2. Identify files to EXCLUDE

Never commit these — warn the user if they exist in the changeset:
- `.env`, `.env.*`, `*.pem`, `*.key`, credentials files
- `node_modules/`, `target/`, `dist/`, `build/` output directories
- `.DS_Store`, `Thumbs.db`, `*.swp`, `*.swo` OS/editor junk
- Large binaries, lock files that weren't intentionally changed

### 3. Analyze and group changes

Read every changed file's diff. Group changes into atomic units where each commit:
- Represents ONE logical change (a feature, a fix, a refactor, a chore)
- Can stand alone — doesn't break the build if applied in isolation
- Contains only related files

Common groupings:
- Type definitions + their implementations
- A new component + its wiring into the app
- Config/dependency changes separate from code changes
- Test files grouped with the code they test

### 4. Determine commit order

Order commits so that:
- Dependencies come before dependents (types before implementations)
- Infrastructure/config changes come first
- Feature code comes after its prerequisites
- Documentation/cleanup comes last

### 5. Present the plan

Show the user a numbered list of proposed commits:

```
Proposed commits (in order):

1. feat(gui): add clean request/response types
   Files: src/gui/types.rs

2. feat(gui): extend AppState with cleaned audio fields
   Files: src/gui/mod.rs

3. feat(gui): add sanitization and cleaned audio endpoints
   Files: src/gui/routes.rs

4. feat(gui): add cleaning API client functions
   Files: gui/src/api/client.ts

5. feat(gui): add CleanPanel component with before/after comparison
   Files: gui/src/components/CleanPanel.tsx, gui/src/App.tsx
```

Ask the user to confirm or adjust before proceeding.

### 6. Execute commits

For each approved commit group:
1. `git reset HEAD` (unstage everything first, only before the first commit)
2. `git add <specific files>` — add only the files for this commit
3. Commit using conventional commit format via HEREDOC:

```bash
git commit -m "$(cat <<'EOF'
<type>(<scope>): <subject>

<optional body explaining why, not what>
EOF
)"
```

4. Run `git status` after each commit to verify

### Conventional Commit Types

| Type | When |
|------|------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `refactor` | Code restructuring, no behavior change |
| `chore` | Build, deps, config, tooling |
| `docs` | Documentation only |
| `style` | Formatting, whitespace, semicolons |
| `test` | Adding or fixing tests |
| `perf` | Performance improvement |
| `ci` | CI/CD changes |

### Rules

- Scope is optional but preferred — use the module/area name (e.g., `gui`, `cli`, `detection`)
- Subject line: imperative mood, lowercase, no period, max 72 chars
- Body: explain WHY the change was made if not obvious from the subject
- NEVER use `--no-verify` or skip hooks
- NEVER amend existing commits unless the user explicitly asks
- NEVER use `git add .` or `git add -A` — always add specific files
- If a commit fails (hooks, etc.), fix the issue and create a NEW commit
- Do NOT add `Co-Authored-By` tags unless the user asks
