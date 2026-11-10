# Doc checks

Validates everything in `docs/`:

1. every ` ```toml ` block parses (using [smol-toml](https://www.npmjs.com/package/smol-toml));
2. code fences are balanced;
3. every relative link points to an existing file;
4. every `#anchor` resolves, using GitHub's heading-slug rules (so links work on github.com).

## Run

```sh
cd tools/doc-checks
npm install        # once
npm run check
```

On Windows PowerShell, where script execution is disabled, use `npm.cmd install` and
`node check.js`.

Run it after every doc change. A non-zero exit code means something is broken.
