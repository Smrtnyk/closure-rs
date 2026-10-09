// ADVANCED entry point of the fabric-srcmap benchmark (bench/README.md): an application entry that
// uses every export of the library, so ADVANCED keeps the library instead of removing it all
// as unused code. SIMPLE compiles the library's own entry point.
import * as lib from "../../bench-cache/fabric-srcmap/dist/index.mjs";
window["fabric"] = lib;
