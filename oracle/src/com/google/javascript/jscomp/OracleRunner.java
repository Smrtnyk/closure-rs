package com.google.javascript.jscomp;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.util.LinkedHashSet;
import java.util.Set;
import com.google.javascript.rhino.StaticSourceFile.SourceKind;

/**
 * A CommandLineRunner that behaves exactly like the stock one, except that (a) the exit code is
 * delivered to the oracle instead of System.exit, (b) every file opened through
 * filenameToOutputStream is recorded (the file is still written by the stock code), and (c)
 * optionally the hidden CommandLineConfig.setTypedAstListInputFilename is set.
 */
final class OracleRunner extends CommandLineRunner {
  final Set<String> outputFilesOpened = new LinkedHashSet<>();

  OracleRunner(String[] args, InputStream in, PrintStream out, PrintStream err) {
    super(args, in, out, err);
  }

  void setTypedAstListInput(String filename) {
    getCommandLineConfig().setTypedAstListInputFilename(filename);
  }

  @Override
  protected OutputStream filenameToOutputStream(String fileName) throws IOException {
    if (fileName != null) {
      outputFilesOpened.add(fileName);
    }
    return super.filenameToOutputStream(fileName);
  }

  /** Source names of the compiler's inputs after dependency sorting/pruning (null if none). */
  java.util.List<String> inputOrder() {
    Compiler c = getCompiler();
    if (c == null) {
      return null;
    }
    java.util.List<String> out = new java.util.ArrayList<>();
    try {
      for (CompilerInput in : c.getInputsInOrder()) {
        out.add(in.getSourceFile().getName());
      }
    } catch (RuntimeException e) {
      return null;
    }
    return out;
  }

  /** createOptions() then setRunOptions(), as AbstractCommandLineRunner.doRun does. */
  CompilerOptions createOptionsForParseDump() throws java.io.IOException {
    CompilerOptions options = createOptions();
    try {
      setRunOptions(options);
    } catch (NullPointerException e) {
      // setRunOptions touches the (not yet created) Compiler only for --json_warnings_file,
      // --error_format=JSON and --skip_normal_outputs, none of which apply to a parse.
      throw new IllegalArgumentException(
          "flag not supported by parse_dump (needs a Compiler in setRunOptions)", e);
    }
    return options;
  }

  /** Source files to mark WEAK when inputs are created (optimize_from_typedast seam). */
  final Set<String> weakInputs = new LinkedHashSet<>();

  /** Chunk fill-file names ("<chunk>$fillFile") to create as empty in-memory inputs. */
  final Set<String> fillInputs = new LinkedHashSet<>();

  @Override
  protected java.util.List<SourceFile> createInputs(
      java.util.List<FlagEntry<JsSourceType>> files,
      java.util.List<JsonFileSpec> jsonFiles,
      boolean allowStdIn,
      java.util.List<JsChunkSpec> jsChunkSpecs)
      throws IOException {
    java.util.List<SourceFile> in = super.createInputs(files, jsonFiles, allowStdIn, jsChunkSpecs);
    if (weakInputs.isEmpty() && fillInputs.isEmpty()) {
      return in;
    }
    for (int i = 0; i < in.size(); i++) {
      String n = in.get(i).getName();
      if (fillInputs.contains(n) && AbstractCompiler.isFillFileName(n)) {
        // Same construction as Compiler.fillEmptyChunks: an empty in-memory placeholder.
        in.set(i, SourceFile.fromCode(n, ""));
      }
    }
    for (SourceFile f : in) {
      if (weakInputs.contains(f.getName())) {
        // The kind createInputs gives a --weakdep file (CommandLineRunner has no --weakdep flag).
        f.setKind(SourceKind.WEAK);
      }
    }
    return in;
  }

  /** Inputs after dependency management, in order, with their SourceKind (null if none). */
  java.util.List<java.util.Map<String, Object>> inputs() {
    Compiler c = getCompiler();
    if (c == null) {
      return null;
    }
    java.util.List<java.util.Map<String, Object>> out = new java.util.ArrayList<>();
    try {
      for (CompilerInput in : c.getInputsInOrder()) {
        java.util.Map<String, Object> m = new java.util.LinkedHashMap<>();
        m.put("name", in.getSourceFile().getName());
        m.put("kind", String.valueOf(in.getSourceFile().getKind()));
        out.add(m);
      }
    } catch (RuntimeException e) {
      return null;
    }
    return out;
  }

  /** Chunk graph after dependency management: name, deps, inputs (name + kind) per chunk. */
  java.util.List<java.util.Map<String, Object>> chunks() {
    Compiler c = getCompiler();
    if (c == null) {
      return null;
    }
    try {
      JSChunkGraph g = c.getChunkGraph();
      if (g == null) {
        return null;
      }
      java.util.List<java.util.Map<String, Object>> out = new java.util.ArrayList<>();
      for (JSChunk ch : g.getAllChunks()) {
        java.util.Map<String, Object> m = new java.util.LinkedHashMap<>();
        m.put("name", ch.getName());
        java.util.List<String> deps = new java.util.ArrayList<>();
        for (JSChunk d : ch.getDependencies()) {
          deps.add(d.getName());
        }
        m.put("deps", deps);
        java.util.List<java.util.Map<String, Object>> ins = new java.util.ArrayList<>();
        for (CompilerInput in : ch.getInputs()) {
          java.util.Map<String, Object> im = new java.util.LinkedHashMap<>();
          im.put("name", in.getSourceFile().getName());
          im.put("kind", String.valueOf(in.getSourceFile().getKind()));
          im.put("fill_file", AbstractCompiler.isFillFileName(in.getSourceFile().getName()));
          ins.add(im);
        }
        m.put("inputs", ins);
        out.add(m);
      }
      return out;
    } catch (RuntimeException e) {
      return null;
    }
  }
}
