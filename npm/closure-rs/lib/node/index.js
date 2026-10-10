// Low-level class that runs the closure-rs binary from Node.
//
// Same public behaviour as the `Compiler` class of google-closure-compiler@20261006.0.0
// (lib/node/index.js), independently implemented. Line numbers below refer to that file.
// Differences, all because closure-rs is a native binary and not a jar:
//   - `javaPath` (the executable that is spawned) defaults to the closure-rs binary, not 'java';
//   - `JAR_PATH` defaults to null, so no JVM arguments are added and `extraCommandArgs` (JVM
//     arguments in the official package) are ignored unless a caller sets JAR_PATH itself;
//   - a spawn failure calls the callback once; the official class also calls it a second time
//     from the 'close' event (Node emits 'error' and then 'close' with a negative code);
//   - a process ended by a signal gives the callback 128 + the signal number (as a shell reports
//     it) instead of null, and stderr names the signal: the official class passes null on, which
//     an `if (exitCode)` check takes for success.
import {spawn} from 'node:child_process';
import {constants as osConstants} from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {bundledPlatforms, getNativeImagePath, thisPlatform} from '../utils.js';

const resolved = getNativeImagePath();

/** The usual cause of a signal, appended to the message for a process ended by one. */
const SIGNAL_HINTS = {
  SIGKILL: ' (on Linux usually the out-of-memory killer: check the memory limit of the machine or ' +
      'container, and how many compiles run at once)',
  SIGILL: ' (an instruction this CPU does not support)',
  SIGSEGV: ' (a crash)',
  SIGBUS: ' (a crash)',
  SIGABRT: ' (an abort)',
  SIGSYS: ' (a system call blocked by a seccomp profile, for example in a container)',
};
/** Where the binary for this machine comes from, for the message of a failed spawn. */
const binaryHint = () => {
  const platforms = bundledPlatforms();
  if (platforms.includes(thisPlatform)) {
    return `It ships in this package under bin/${thisPlatform}/; CLOSURE_RS_BINARY can point at ` +
        'a binary instead.';
  }
  return `This package has no binary for ${thisPlatform} (it has ` +
      `${platforms.length ? 'binaries for ' + platforms.join(', ') : 'none'}); CLOSURE_RS_BINARY ` +
      'can point at a closure-rs binary built from source.';
};

/**
 * The executable `run()` spawns: the closure-rs binary from the platform package (or
 * CLOSURE_RS_BINARY). When neither exists, 'closure-rs' is looked up on PATH, just as the official
 * package falls back to 'java' on PATH.
 * @type {string}
 */
export const javaPath = resolved || 'closure-rs';

const packageRoot = path.resolve(fileURLToPath(new URL('../..', import.meta.url)));

export default class Compiler {
  /**
   * lines 36-59.
   * @param {Object<string,*>|Array<string>} args
   * @param {Array<string>=} extraCommandArgs JVM arguments; used only when JAR_PATH is set.
   */
  constructor(args, extraCommandArgs) {
    this.commandArguments = [];
    this.extraCommandArgs = extraCommandArgs;
    this.JAR_PATH = null;
    this.javaPath = javaPath;

    if (Array.isArray(args)) {
      // An array is passed through verbatim.
      for (const arg of args) {
        this.commandArguments.push(arg);
      }
    } else {
      // An object becomes one flag per key; an array value repeats the flag once per item.
      for (const key of Object.keys(args)) {
        const val = args[key];
        const items = Array.isArray(val) ? val : [val];
        for (const item of items) {
          this.commandArguments.push(this.formatArgument(key, item));
        }
      }
    }

    /** @type {function(...*)|null} */
    this.logger = null;

    /** @type {Object|undefined} Options for child_process.spawn. */
    this.spawnOptions = undefined;
  }

  /**
   * lines 62-115. Spawns the compiler and returns the ChildProcess. With a callback, stdout and
   * stderr are collected as UTF-8 and the callback gets (exitCode, stdout, stderr) once the process
   * closes; on a non-zero exit the full command line is put in front of stderr.
   * @param {function(number, string, string)=} callback
   * @return {!ChildProcess}
   */
  run(callback) {
    if (this.JAR_PATH) {
      // Only reached when a caller points JAR_PATH at a real jar (and javaPath at java): then
      // behave like the official class and run the jar.
      this.commandArguments.unshift(
          '-XX:+IgnoreUnrecognizedVMOptions',
          '--sun-misc-unsafe-memory-access=allow',
          '-jar',
          this.JAR_PATH);
      if (this.extraCommandArgs) {
        this.commandArguments.unshift(...this.extraCommandArgs);
      }
    }

    if (this.logger) {
      this.logger(this.getFullCommand() + '\n');
    }
    const child = spawn(this.javaPath, this.commandArguments, this.spawnOptions);

    if (callback) {
      let out = '';
      let err = '';
      let done = false;
      const finish = (code, stderr) => {
        if (!done) {
          done = true;
          callback(code, out, stderr);
        }
      };
      if (child.stdout) {
        child.stdout.setEncoding('utf8');
        child.stdout.on('data', (chunk) => {
          out += chunk;
        });
        child.stdout.on('error', (e) => {
          err += e.toString();
        });
      }
      if (child.stderr) {
        child.stderr.setEncoding('utf8');
        child.stderr.on('data', (chunk) => {
          err += chunk;
        });
      }
      child.on('close', (code, signal) => {
        if (code === null && signal) {
          const number = osConstants.signals[signal] || 0;
          finish(128 + number, this.prependFullCommand(
              `${err}closure-rs was terminated by signal ${signal}${SIGNAL_HINTS[signal] || ''}.`));
          return;
        }
        finish(code, code !== 0 ? this.prependFullCommand(err) : err);
      });
      child.on('error', (e) => {
        finish(1, this.prependFullCommand(
            'Process spawn error. Is the closure-rs binary installed? ' + binaryHint() + '\n' +
            e.message));
      });
    }

    return child;
  }

  /**
   * lines 121-123.
   * @return {string}
   */
  getFullCommand() {
    return `${this.javaPath} ${this.commandArguments.join(' ')}`;
  }

  /**
   * lines 129-131.
   * @param {string} msg
   * @return {string}
   */
  prependFullCommand(msg) {
    return `${this.getFullCommand()}\n\n${msg}\n\n`;
  }

  /**
   * lines 138-147. camelCase keys become snake_case, a leading '--' on the key is dropped, a
   * null/undefined value gives the bare flag and anything else gives `--key=value` (booleans as
   * `--key=true` / `--key=false`). No shell is involved, so values need no quoting.
   * @param {string} key
   * @param {*=} val
   * @return {string}
   */
  formatArgument(key, val) {
    const name = key.replace(/[A-Z]/g, (c) => '_' + c.toLowerCase()).replace(/^--/, '');
    return val === undefined || val === null ? `--${name}` : `--${name}=${val}`;
  }
}

// Static paths. The official class has none at runtime, but @types/google-closure-compiler
// declares compiler.JAR_PATH, compiler.COMPILER_PATH and compiler.CONTRIB_PATH, so code written
// against those types finds them here.
Compiler.JAR_PATH = null;
Compiler.COMPILER_PATH = javaPath;
Compiler.CONTRIB_PATH = path.join(packageRoot, 'contrib');
Compiler.EXTERNS_PATH = path.join(packageRoot, 'externs');
