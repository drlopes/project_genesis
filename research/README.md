# Research

Everything we read, watch, or study, recorded so it can be found again and so its lessons are not
lost. Three layers, from raw to distilled:

| Layer | Where | Committed | Contents |
|---|---|---|---|
| **Library** | [`library/`](library/README.md) | **never** | local copies: PDFs, saved articles, downloaded videos, cloned example code |
| **Source cards** | [`sources/`](sources/) | yes | one Markdown card per source: citation, link, why it matters, our notes |
| **Knowledge** | [`../knowledge/`](../knowledge/README.md) | yes | distilled understanding, in our own words, citing cards |

The architecture docs in [`docs/`](../docs/README.md) are the **decisions for this game**.
Knowledge notes are the **understanding behind them**: more general, and they may cover options we
did not choose.

## Copyright rules

Research material is usually copyrighted. The repository is public. Therefore:

1. **No raw material in git.** PDFs, article text, video files, slides, figures, and third-party
   code stay in `library/`, which is ignored.
2. **Write in our own words.** Cards and knowledge notes paraphrase and summarise. A short quote
   (a sentence or two) is fine when the exact wording matters, always attributed and marked as a
   quote.
3. **No copied figures.** Redraw a diagram in our own form if it is needed.
4. **Code is referenced, not vendored.** Link to the repository and a commit or tag, note the
   licence, and describe the technique. Our implementation is written from the understanding, not
   pasted. Even permissively licensed code is linked rather than copied, to keep provenance clear.
5. **Maths and algorithms are not copyrightable;** explaining them in our words, with citation, is
   exactly what the knowledge base is for.

## Adding a source

1. Copy [`sources/_TEMPLATE.md`](sources/_TEMPLATE.md) to `sources/<id>.md`. The ID is
   `<first-author-or-org>-<year>-<short-title>`, lowercase with hyphens, e.g.
   `perlin-2002-improving-noise`. It never changes once created.
2. If you keep a local copy, save it as `library/<id>.<ext>` (or a folder `library/<id>/` for
   cloned code) and record its SHA-256 in the card, so it can be verified or re-found later:
   ```powershell
   (Get-FileHash research/library/perlin-2002-improving-noise.pdf).Hash
   ```
3. Fill in the card. Set **Status** as you go: `to-read` → `read` → `distilled`.
4. When it teaches something we will use, distil it into a note in `knowledge/` and link back.

## Statuses

| Status | Meaning |
|---|---|
| `to-read` | captured, not yet studied |
| `read` | studied; notes in the card |
| `distilled` | its useful content is in a knowledge note |
| `superseded` | replaced by a better source; kept for history |

## Backups

`library/` is not on GitHub. Back it up separately (an external drive or a cloud folder). The cards
record the original URL and the file hash, so a lost library can be rebuilt by re-downloading.
