# Open questions

## Problem context

- [ ] TODO(owner): The paper and report present this as an academic project with four student authors. Can it be presented as ByteHint work with "Built by the ByteHint team" credit, and do the co-authors agree? This decides whether the video can be made at all. (`overview.md`, `redaction-check.md`)
- [ ] TODO(owner): Was it driven by a real workload or user, or built as a research and learning project? (`overview.md`)
- [ ] TODO(owner): Can the product name be shown on screen, or should it be called "the database"?

## Outcomes and numbers

- [ ] TODO(owner): Any users, deployments, or results beyond the paper's single-machine benchmark? (`video-script.md`)
- [ ] TODO(owner): Are the raw `redis-benchmark` runs behind `paper/chapters/chapter6.tex` saved anywhere, so the numbers can be re-run before quoting them?
- [ ] TODO(owner): Which CTA and ByteHint link close the video? (`video-script.md`)

## Things the code could not confirm

- [ ] TODO(owner): Is CI green? Locally, the build has unused-import warnings that `clippy -D warnings` would reject, and the workflow uses deprecated actions. (`technical-depth.md`)
- [ ] TODO(owner): Which platform hosts the database container used by the demos, and is it still running? (`stack.md`, `demo-prep.md`)
- [ ] TODO(owner): Is the Render bridge still deployed? (`demo-prep.md`)
- [ ] TODO(owner): Keep the Trackmania telemetry demo (restore from commit `e3ee042`) or drop it? (`overview.md`)
- [ ] TODO(owner): What is `important documents/website.46af363f.mp4`? (`demo-prep.md`, `redaction-check.md`)
- [ ] TODO(owner): Have the Supabase and hosted-database passwords in `demo/*/server.js` been rotated, and is the GitHub repo public? (`redaction-check.md`)

## CMS fields that need a human

- [ ] TODO(owner): `origin` and credit: confirm the project can be published as ByteHint work (academic co-authors). Don't publish the entry until this is resolved.
- [ ] TODO(owner): `industry`: none fits clearly (developer tools or software infrastructure?). Pick one from the Industry list.
- [ ] TODO(owner): `headlineMetrics`: the +42.8% figure comes from the project paper, not from a run in this session. Re-run `scripts/benchmark.ps1` before publishing it.
- [ ] TODO(owner): `thumbnail`, `heroImage`, `seo.metaImage`, `screenshots`: capture S1 to S5 from `demo-prep.md`.
- [ ] TODO(owner): `videoFull.url`, `videoShort.url`: add after recording, and correct both transcript drafts.
- [ ] TODO(owner): `links` / `publicStats`: is the repo public, and may ByteHint link to it? None were added.
- [ ] TODO(owner): testimonial: none was written.
