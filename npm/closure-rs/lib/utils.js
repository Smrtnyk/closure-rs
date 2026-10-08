// Locating the closure-rs binary and choosing a "platform".
//
// Behaviour pinned to google-closure-compiler@20261005.0.0 lib/utils.js (getNativeImagePath, lines
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

/** Executable file name inside a platform package (scripts/npm_pack_platform.mjs puts it in bin/). */
const exeName = process.platform === 'win32' ? 'closure-rs.exe' : 'closure-rs';

/** Name of the optional platform package for this OS and CPU, e.g. closure-rs-linux-x64. */
export const platformPackageName = `${packageName}-${process.platform}-${process.arch}`;

/** Binary bundled in this package: bin/<platform>-<arch>/closure-rs[.exe] (scripts/npm_pack_main.mjs --binaries). */
export const bundledBinaryPath = path.join(
    path.dirname(fileURLToPath(import.meta.url)), '..', 'bin', `${process.platform}-${process.arch}`, exeName);

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
