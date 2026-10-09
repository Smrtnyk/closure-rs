// Paths for the Node scripts, as scripts/paths.sh sets them for the shell scripts:
//   ROOT    the main checkout, also from a git worktree; $CLOSURE_RS_ROOT overrides; without git
//           it is the parent of scripts/.
//   WT      a directory for extra git worktrees: $CLOSURE_RS_WT, default closure-rs-wt next to ROOT.
//   SSD_WT  optional build disk for worktrees: $CLOSURE_RS_SSD_WT, default '' (none).
// and the Java reference (docs/PORTING.md §9): REF, the row of scripts/references.tsv (read from
// this checkout) named by $CLOSURE_RS_REF, default its `default` row (paths relative to ROOT), and
// the same values as paths.sh exports (paths absolute under ROOT): REF_TAG, REF_COMMIT,
// REF_JAR_SHA256 ("-" = not pinned yet), REF_GOLDEN_TAG ("ref-<sha8>" or "-"), REF_SRC,
// REF_RECORDING_WS, REF_JAR, ORACLE_JAR.
// `node scripts/paths.mjs` prints the resolved values.
import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {basename, dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
export const REGISTRY = join(HERE, 'references.tsv');

function mainCheckout() {
  if (process.env.CLOSURE_RS_ROOT) return process.env.CLOSURE_RS_ROOT;
  try {
    const common = execFileSync('git', ['-C', HERE, 'rev-parse', '--path-format=absolute', '--git-common-dir'],
        {encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore']}).trim();
    if (basename(common) === '.git') return dirname(common);
  } catch {
    // not a git checkout
  }
  return resolve(HERE, '..');
}

export const ROOT = mainCheckout();
export const WT = process.env.CLOSURE_RS_WT || join(dirname(ROOT), 'closure-rs-wt');
export const SSD_WT = process.env.CLOSURE_RS_SSD_WT || '';

/** {refs: {tag: row}, defaultTag} of a references.tsv; row paths are relative to ROOT. */
export function loadRegistry(path = REGISTRY) {
  const refs = {};
  let defaultTag = '';
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (line.startsWith('#') || !line.trim()) continue;
    const c = line.replace(/\r$/, '').split('\t');
    if (c[0] === 'default') {
      defaultTag = c[1];
    } else if (c.length >= 7) {
      const [tag, commit, jar_sha256, src, recording_ws, jar, oracle_jar] = c;
      refs[tag] = {tag, commit, jar_sha256, src, recording_ws, jar, oracle_jar};
    }
  }
  return {refs, defaultTag};
}

/** The registry row of `tag`, default $CLOSURE_RS_REF, else the registry's default. */
export function reference(tag) {
  const {refs, defaultTag} = loadRegistry();
  const t = tag || process.env.CLOSURE_RS_REF || defaultTag;
  if (!Object.hasOwn(refs, t)) {
    throw new Error(`scripts/paths.mjs: unknown reference '${t}' (CLOSURE_RS_REF; tags in ${REGISTRY})`);
  }
  return refs[t];
}

export const REF = reference();
export const REF_TAG = REF.tag;
export const REF_COMMIT = REF.commit;
export const REF_JAR_SHA256 = REF.jar_sha256;
export const REF_GOLDEN_TAG = REF.jar_sha256 === '-' ? '-' : 'ref-' + REF.jar_sha256.slice(0, 8);
export const REF_SRC = join(ROOT, REF.src);
export const REF_RECORDING_WS = join(ROOT, REF.recording_ws);
export const REF_JAR = join(ROOT, REF.jar);
export const ORACLE_JAR = join(ROOT, REF.oracle_jar);

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const v = {ROOT, REF_TAG, REF_COMMIT, REF_JAR_SHA256, REF_GOLDEN_TAG, REF_SRC, REF_RECORDING_WS,
    REF_JAR, ORACLE_JAR};
  for (const [k, x] of Object.entries(v)) console.log(`${k}=${x}`);
}
