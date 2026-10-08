// Gulp plugin: NOT implemented in closure-rs (a stub with the official signature).
//
// The official plugin (google-closure-compiler@20261005.0.0 lib/gulp/index.js, default export at
// line 287) is not a thin layer over the compiler class: it converts Vinyl files to the compiler's
// --json_streams format and back and merges source maps, using the vinyl, vinyl-sourcemaps-apply
// and chalk packages. closure-rs ships no third-party dependencies, so the plugin is not provided.
// Calling it throws; use the `compiler` class instead.
export const GULP_UNSUPPORTED =
    'closure-rs does not implement the gulp plugin of google-closure-compiler. ' +
    'Run the compiler class instead (new compiler(flags).run(callback)), ' +
    'or keep the official google-closure-compiler package for gulp builds.';

export default function gulp(compilationOptions, pluginOptions) {
  throw new Error(GULP_UNSUPPORTED);
}
