# Art

| Folder | Holds | Committed |
|---|---|---|
| [`concept/`](concept/README.md) | our concept art: own work, or commissioned work we have the rights to | yes, via **Git LFS** |
| [`references/`](references/README.md) | third-party images: moodboards, screenshots, photos | **no** (ignored) |

## Why Git LFS

Every version of a binary file committed to plain git stays in the history forever, and every
clone downloads all of it. With LFS, git stores a small pointer and the image lives in LFS
storage; clones fetch only the versions they check out.

- LFS is enabled for this repository (`git lfs install --local` once per clone).
- `.gitattributes` routes image and art-tool formats **under `art/`** through LFS. Nothing outside
  `art/` uses LFS.
- GitHub's free plan has a limited LFS storage and bandwidth quota; check usage in the account
  settings if the collection grows large (layered PSDs are the usual culprit).
- CI does not download LFS files, so CI runs do not use the bandwidth quota.

## Rights

Only commit art we have the right to publish: the repository is public. Commissioned art needs a
licence that allows this; record it in the index. Anything else is a reference and goes in
`references/`.
