// Locating the closure-rs binary and choosing a "platform".
//
// Behaviour pinned to google-closure-compiler@20261006.0.0 lib/utils.js (getNativeImagePath, lines
// 28-42; getFirstSupportedPlatform, lines 48-67). That package picks between a native binary from
// an optional per-platform package and the Java jar. closure-rs has only the native binary, so
// both 'native' and 'java' select it.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';

const require = createRequire(import.meta.url);

/** Name of this package as published (npm/config.json "name"); platform packages append -<platform>-<arch>. */
const packageName = JSON.parse(
  fs.readFileSync(new URL('../package.json', import.meta.url), 'utf8')).name;

/** Executable file name, under bin/<platform>-<arch>/ here and under bin/ in a platform package. */
const exeName = process.platform === 'win32' ? 'closure-rs.exe' : 'closure-rs';

/** Name of the optional platform package for this OS and CPU, e.g. closure-rs-linux-x64. */
export const platformPackageName = `${packageName}-${process.platform}-${process.arch}`;

/** Folder of the bundled binaries: bin/<platform>-<arch>/closure-rs[.exe] (scripts/npm_pack_main.mjs --binaries). */
const binDir = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'bin');

/**
 * Platforms whose binary also runs on this one, after its own: Windows on Arm runs x64 programs
 * (emulated) when the package has no win32-arm64 binary.
 */
const RUNS_ALSO = {'win32-arm64': ['win32-x64']};

/** This machine as <platform>-<arch>, the name of its folder under bin/. */
export const thisPlatform = `${process.platform}-${process.arch}`;

/**
 * Binary bundled in this package for this machine: bin/<platform>-<arch>/closure-rs[.exe], or the
 * binary of a platform in RUNS_ALSO when only that one exists.
 */
export const bundledBinaryPath = [thisPlatform, ...(RUNS_ALSO[thisPlatform] || [])]
    .map((p) => path.join(binDir, p, exeName))
    .find((p) => fs.existsSync(p)) || path.join(binDir, thisPlatform, exeName);

/**
 * The <platform>-<arch> folders under bin/: the systems this package has a binary for.
 * @return {!Array<string>}
 */
export const bundledPlatforms = () => {
  try {
    return fs.readdirSync(binDir).filter((p) => fs.existsSync(
        path.join(binDir, p, p.startsWith('win32-') ? 'closure-rs.exe' : 'closure-rs'))).sort();
  } catch {
    return [];
  }
};

/**
 * Absolute path of the closure-rs binary, or undefined when none is installed.
 * CLOSURE_RS_BINARY (an explicit path, for local testing) wins over the binary bundled in this
 * package, which wins over a separate platform package.
 * @return {string|undefined}
 */
export const getNativeImagePath = () => {
  const fromEnv = process.env.CLOSURE_RS_BINARY;
  if (fromEnv) {
    return path.resolve(fromEnv);
  }
  if (fs.existsSync(bundledBinaryPath)) {
    return bundledBinaryPath;
  }
  try {
    return require.resolve(`${platformPackageName}/bin/${exeName}`);
  } catch {}
};

/**
 * Same contract as the official helper: returns the first usable entry of `platforms` and throws
 * when there is none. 'native' and 'java' (any case) are usable and both mean the closure-rs
 * binary, so the result is always 'native'; other entries (e.g. 'javascript') are skipped.
 * @param {!Array<string>} platforms
 * @return {string}
 */
export const getFirstSupportedPlatform = (platforms) => {
  const found = platforms.some((p) => ['native', 'java'].includes(String(p).toLowerCase()));
  if (!found) {
    throw new Error('No supported platform for closure-compiler found.');
  }
  return 'native';
};
