// ADVANCED entry point of the three-bundle benchmark (bench/README.md): an application entry that
// uses every export of the library, so ADVANCED keeps the library instead of removing it all
// as unused code. SIMPLE compiles the library's own entry point.
import * as lib from "../../bench-cache/three-bundle/build/three.module.js";
window["three"] = lib;
