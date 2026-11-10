// Validates every Markdown file git tracks (or would track) in the repository:
//   1. every ```toml block parses
//   2. code fences are balanced
//   3. every relative link points at an existing file
//   4. every #anchor resolves, using GitHub's heading-slug rules
//
// Usage: npm install && npm run check   (from tools/doc-checks)
// Exit code is non-zero if any check fails.

const fs = require('fs');
const path = require('path');
const { parse } = require('smol-toml');

const { execSync } = require('child_process');

// Paths are forward-slash, relative to the repository root. Using git's view (tracked plus new,
// minus ignored) means local-only folders such as research/library/ are never scanned.
const docsDir = path.resolve(__dirname, '..', '..');
const files = execSync('git ls-files --cached --others --exclude-standard -- "*.md"', {
  cwd: docsDir,
  encoding: 'utf8',
})
  .split('\n')
  .map(f => f.trim())
  .filter(f => f && !f.includes('node_modules/') && fs.existsSync(path.join(docsDir, f)))
  .sort();
const read = f => fs.readFileSync(path.join(docsDir, f), 'utf8');
const lineOf = (text, index) => text.slice(0, index).split(/\r?\n/).length;

// GitHub-compatible slug: lowercase, drop everything except letters, marks,
// numbers, connector punctuation, spaces and hyphens, then each space -> '-'.
// Spaces are not collapsed, so "A — B" becomes "a--b".
function slug(heading) {
  return heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{M}\p{N}\p{Pc} -]/gu, '')
    .replace(/ /g, '-');
}

// Strip fenced code so headings/links inside code blocks are ignored.
function stripFences(text) {
  return text.replace(/^```[\s\S]*?^```/gm, m => m.replace(/[^\n]/g, ' '));
}

const failures = [];
let tomlCount = 0;
const anchors = {};

for (const file of files) {
  const text = read(file);

  // 1. TOML blocks
  const re = /```toml\r?\n([\s\S]*?)```/g;
  let m, i = 0;
  while ((m = re.exec(text))) {
    i++;
    tomlCount++;
    try {
      parse(m[1]);
    } catch (e) {
      failures.push(`${file}:${lineOf(text, m.index)} TOML block ${i}: ${e.message.split('\n')[0]}`);
    }
  }

  // 2. Fences
  const fences = (text.match(/^```/gm) || []).length;
  if (fences % 2 !== 0) failures.push(`${file}: unbalanced code fences (${fences})`);

  // Collect anchors, with GitHub's -1, -2 suffixes for duplicates.
  const seen = new Map();
  anchors[file] = new Set();
  for (const h of stripFences(text).matchAll(/^#{1,6}\s+(.+?)\s*#*\s*$/gm)) {
    const base = slug(h[1].replace(/`/g, ''));
    const n = seen.get(base) || 0;
    anchors[file].add(n === 0 ? base : `${base}-${n}`);
    seen.set(base, n + 1);
  }
}

// 3 + 4. Links and anchors
for (const file of files) {
  const text = stripFences(read(file));
  for (const m of text.matchAll(/\]\(((?:\.\/)?[^)\s#]*)(#[^)\s]*)?\)/g)) {
    const target = m[1];
    const anchor = m[2] ? decodeURIComponent(m[2].slice(1)) : null;
    if (/^[a-z]+:/i.test(target)) continue; // external URL
    const where = `${file}:${lineOf(text, m.index)}`;
    // Resolve relative to the linking file's own folder, then express relative to docs/.
    const targetFile = target === ''
      ? file
      : path.posix.normalize(path.posix.join(path.posix.dirname(file), target));
    if (target !== '' && !fs.existsSync(path.join(docsDir, targetFile))) {
      failures.push(`${where}: missing file ${target}`);
      continue;
    }
    if (anchor && anchors[targetFile] && !anchors[targetFile].has(anchor)) {
      failures.push(`${where}: missing anchor #${anchor} in ${targetFile}`);
    }
  }
}

for (const f of failures) console.log(f);
console.log(
  failures.length === 0
    ? `OK: ${files.length} Markdown files, ${tomlCount} TOML blocks, fences, links and anchors valid`
    : `FAILED: ${failures.length} problem(s) across ${files.length} Markdown files`
);
process.exit(failures.length ? 1 : 0);
