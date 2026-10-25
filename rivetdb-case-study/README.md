# Showcase kit: Redis-compatible database in Rust

Project type: a database server (an in-memory key-value store that speaks the Redis protocol), with three demo apps on top. Built by the ByteHint team, pending the credit question in `open-questions.md`.

## Files

| File | What it is for |
|---|---|
| `overview.md` | What it is, the problem, the solution, features with file paths, journeys, key decisions, hard parts, failure handling |
| `stack.md` | Every dependency (Rust crate and demo package) with versions, plus a read-aloud "stack at a glance" |
| `technical-depth.md` | Concept-by-concept map with evidence, the real test count and coverage figure, security gaps, recommended roadmap |
| `video-script.md` | 4-minute talking points (terminal + two demo apps), a 90 to 120 second cut, and what not to claim |
| `demo-prep.md` | Local run commands that never touch hosted services, plus seed commands |
| `redaction-check.md` | Committed credentials and everything else that must stay off screen |
| `open-questions.md` | All TODO(owner) items in one checklist, including CMS fields that need a human |
| `cms-entry.json` | Paste-ready Strapi Case Study entry. Media fields are `FILE:` placeholders. |

## Diagrams (`diagrams/`)

All six rendered to `.svg` (transparent) and `.png` (white, 3x) without errors.

| File | Shows |
|---|---|
| `architecture` | Clients, TCP and RESP layer, router, background tasks, DashMap keyspace, AOF, metrics |
| `infrastructure` | CI jobs, container images, Render, hosted database, Supabase |
| `data-model` | Server state, value types, expiry heap, schema registry, and the whiteboard's Postgres tables |
| `flow-command` | One `SET` from bytes to reply, including the async AOF hand-off |
| `flow-whiteboard` | Create, join, draw, and archive a board |
| `auth-flow` | Per-connection `AUTH`, `NOAUTH`, exempt commands |

Using the diagrams elsewhere:
- **Excalidraw:** paste the `.mmd` contents into Excalidraw's Mermaid import for editable shapes, or drag in the SVG.
- **Miro:** paste the Mermaid into Miro's diagram-from-code option, or drop in the SVG or PNG.
- **Drawing manually:** use the PNG as reference.
- **CMS upload:** upload the SVG (PNG as fallback) to each diagram's image field, and attach the `.mmd` as `sourceFile`. The `.mmd` text is already pasted into `mermaidSource` in `cms-entry.json`.

Re-render after editing:
```bash
npx -y @mermaid-js/mermaid-cli -i diagrams/architecture.mmd -o diagrams/architecture.svg -b transparent
```
