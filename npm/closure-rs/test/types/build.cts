// The same API from a CommonJS TypeScript build file (.cts, module nodenext: require of the ESM package).
import ClosureCompiler = require('google-closure-compiler');
import {compiler} from 'google-closure-compiler';

new compiler({js: 'in.js', compilation_level: 'ADVANCED'}).run((exitCode, stdout, stderr) => {
  console.log(exitCode, stdout, stderr);
});
new ClosureCompiler.compiler(['--js', 'in.js']).run();
new ClosureCompiler.default({js: 'in.js'}).run();
