# Redaction check

Scope: git-tracked files, untracked working files, `paper/`, `important documents/`, `demo/`, and the root folder. No secret values are reproduced here.

No client name, brand, or domain was found. This is not a client project. The paper and report present it as an academic project with four student authors and an institution logo (see items 6 and 7, and `open-questions.md`).

## Must act before recording (and before anyone else sees the repo)

| # | Finding | Where | Action |
|---|---|---|---|
| 1 | **Postgres connection string with password**, used as a fallback default | `demo/whiteboard-react/server.js` line 35 (tracked, committed in `91f6011`) | Rotate the Supabase database password now. Remove the fallback and require `DATABASE_URL`. The value stays in git history, so rotation is what counts. If the GitHub repo is public, treat the credential as already exposed. |
| 2 | **Hosted database host and password**, used as fallback defaults | `demo/whiteboard-react/server.js` lines 23 to 27, `demo/trackmania/server.js` lines 14 to 18 (tracked, committed in `91f6011` and `e3ee042`) | Change the hosted server's `RIVETDB_PASSWORD`. Remove the defaults. `render.yaml` says "instead of committing secrets", but the defaults are committed anyway. |
| 3 | Command logging prints `AUTH` passwords | `src/main.rs` (`info!` "CMD" with `args`) | Don't show server logs while a password is set. Fix in code before any shared deployment. |
| 4 | Code dump containing email addresses and a private IP | `rivetdb_context.txt` (500 KB, tracked) | Don't show it. Remove it from the repo. |
| 5 | Commit author email | git log (all commits) | Don't show `git log` on screen. |
| 6 | Student names on certificates, a signed report, and acknowledgements | `paper/report/cert_*.pdf`/`.tex`, `*_cert_ack.*`, `main_signed.pdf`, `paper1realcert.pdf`, `paper2realcert.pdf`, `title_page.docx` (ignored by `paper/report/` in `.gitignore`, still on disk) | Never show. Credit in the video is "the ByteHint team". |
| 7 | Institution logo and author block | `paper/sits_logo.png` (tracked), `paper/report/sits_logo.png`, `\author{...}` in `paper/rivetdb_paper.tex` and `rivetdb_paper2.tex` (tracked) | Don't show the paper's title page or the logo. If you use benchmark tables, re-typeset them without the author block. |
| 8 | Meme image and unrelated files | `important documents/sab-apne-apne-freaky-images-daalo-v0-*.webp`, `png.png`, `website.46af363f.mp4`, two PDFs (tracked) | Keep off screen. Check the mp4 before using it. Consider removing them from the repo. |

## Review before recording

| # | Finding | Where | Action |
|---|---|---|---|
| 9 | Deployed bridge URL on Render (contains the service name) | `demo/trackmania/forwarder.mjs` line 22, `render.yaml` | Don't show it. Not a secret. |
| 10 | Personal GitHub remote | `git remote` (a personal account) | Don't show remotes. |
| 11 | Local absolute paths (`d:\dev\rivetDb`) | `demo/whiteboard/README.md` | Harmless, but avoid showing it. |
| 12 | Binaries and archives in the repo | `main.exe`, `main.pdb` (tracked), `src.zip` (tracked), `rivetDb.zip` (14 MB), `appendonly.aof` (173 MB, ignored) | Keep off screen. The AOF may hold earlier test data. Don't open it. |
| 13 | Build and test logs | `build_errors.txt`, `build_log.txt`, `build_output.txt`, `test_output.txt` (tracked) | Contain local paths. Not needed on screen. |
| 14 | "Redis" name and comparisons | code comments, paper, video script | Use it only to describe compatibility ("speaks the Redis protocol") and the benchmark comparison. Don't imply affiliation or use Redis logos. |
| 15 | Trackmania / Openplanet names | `demo/trackmania/` | Third-party game and tool names. Don't show their logos. The telemetry demo is out of the video anyway. |
| 16 | Dummy names in demo seed data | `demo-prep.md` (Asha, Vikram, Meera) | Fictional, fine to show. |

## Not found

No API keys for paid services, no payment data, no real user records in tracked source. The whiteboard archive in Supabase may hold drawings and display names from past sessions; don't open it on camera.
