# Lessons Learned

> Updated by the agent after any user correction. Reviewed at session start to avoid repeating mistakes.

## Patterns

### 2026-11-15 — Feature hidden until fully set up looked broken
- **What happened**: The Spotify heart only rendered after the user connected a Spotify developer app, so the user saw nothing and reported it as not working.
- **Root cause**: Gated the control's visibility on its prerequisites instead of showing it with a path to set it up.
- **Rule**: Show new controls in a dimmed/setup state that explains and links to the missing step; never make a feature invisible until configured.

### 2026-11-15 — Dense locale date in a tight status row
- **What happened**: The notch date used the locale's weekday + month names ("Dom 15 De Nov") and crowded the row; the user wanted dd/mm.
- **Root cause**: Picked a verbose format without checking the space the row actually has.
- **Rule**: In compact UI (notch, dock), default to the shortest numeric form and check it against a real screenshot of the row.

### 2026-11-15 — Over-eager dismissal of a full-screen page
- **What happened**: The Orbix lock screen closed on any mouse movement; the user wanted only Enter or a click.
- **Root cause**: Copied the "any input wakes it" behavior of real lock screens without asking how the user wanted to dismiss it.
- **Rule**: For pages the user deliberately looks at (lock, focus, screensaver), dismiss only on an explicit action unless told otherwise.

<!-- 
Format for new entries:

### [Date] — [Short description]
- **What happened**: [What went wrong]
- **Root cause**: [Why it went wrong]
- **Rule**: [The principle to follow next time]

Example:

### 2025-01-15 — Used deprecated API method
- **What happened**: Called `db.query()` which was removed in v3.
- **Root cause**: Didn't check the migration guide before updating the dependency.
- **Rule**: Always read changelogs/migration guides before upgrading major versions.
-->
