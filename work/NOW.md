# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**Set up the work structure** — work tracking, changelog, research library and knowledge base,
concept-art storage.

- [x] `work/` with process rules, `NOW.md`, `BUGS.md`, `IDEAS.md`
- [x] `CHANGELOG.md`, backfilled from git history
- [x] `research/` (source cards, ignored local library) and `knowledge/` (distilled notes)
- [x] `art/` with Git LFS for concept art; third-party references ignored
- [x] `AGENTS.md`, VS Code, and doc checks updated for the new folders
- [ ] User reviews the structure

## Where we stopped

Structure is in place and committed. Next step: user review, then start **E3** (below).

## Waiting on the user

1. **Rewrite history to drop 11 MB of build artefacts** committed in `61e6dc6` (already removed
   from the tree; still in history). Requires a force-push to GitHub. Low risk while the repo has a
   single user. Decide: rewrite now, or accept the 11 MB.
2. **Budget table review** in [docs/21](../docs/21-phase-1-spike-plan.md#reference-hardware-and-budgets)
   before E3 starts (60 fps at 1440p, memory and VRAM targets).

## Up next

1. **E3 — LOD spike app.** First Bevy build: a minimal app streaming cube-sphere patches with a fly
   camera and scripted paths. Measures E9 (build iteration) from the first build.
   ([docs/21](../docs/21-phase-1-spike-plan.md#e3--lod-bands-and-quadtree))
2. **E4 — Streaming throughput**, on the same app.
3. **E5 — Coordinate precision.**
4. **E6 — Collision and physics**, then **E7 — Voxels**.
5. **E8 — Classification look**, using the inspector renders.
