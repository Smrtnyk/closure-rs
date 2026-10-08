// ADVANCED entry point of the d3-hierarchy benchmark (bench/README.md): an application entry that
// uses every export of the library, so ADVANCED keeps the library instead of removing it all
// as unused code. SIMPLE compiles the library's own entry point.
import * as lib from "../../bench-cache/d3-12/node_modules/d3-hierarchy/src/index.js";
window["d3-hierarchy"] = lib;
