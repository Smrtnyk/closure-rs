// Grunt plugin: NOT implemented in closure-rs (a stub with the official signature).
//
// The official task (google-closure-compiler@20261006.0.0 lib/grunt/index.js, default export at
// line 32) streams the source files through the gulp plugin, which closure-rs does not provide
// (see ../gulp/index.js). Calling it throws; use the `compiler` class instead.
export const GRUNT_UNSUPPORTED =
    'closure-rs does not implement the grunt plugin of google-closure-compiler. ' +
    'Run the compiler class instead (new compiler(flags).run(callback)), ' +
    'or keep the official google-closure-compiler package for grunt builds.';

export default function grunt(gruntInstance, pluginOptions) {
  throw new Error(GRUNT_UNSUPPORTED);
}
