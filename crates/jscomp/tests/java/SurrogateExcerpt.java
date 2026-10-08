package com.google.javascript.jscomp;
import com.google.debugging.sourcemap.proto.Mapping.OriginalMapping;
import java.io.*;
import java.nio.charset.StandardCharsets;
public final class SurrogateExcerpt {
  public static void main(String[] a) throws Exception {
    final SourceFile f = SourceFile.fromCode("a.js", "var x = '\uD800';\nvar y = '\uDC00😀z';\n");
    SourceExcerptProvider p = new SourceExcerptProvider() {
      @Override public String getSourceLine(String n, int l) { return f.getLine(l); }
      @Override public Region getSourceLines(String n, int l, int len) { return f.getLines(l, len); }
      @Override public Region getSourceRegion(String n, int l) { return f.getRegion(l); }
      @Override public OriginalMapping getSourceMapping(String n, int l, int c) { return null; }
    };
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    PrintStream ps = new PrintStream(out, true, StandardCharsets.UTF_8);
    DiagnosticType t = DiagnosticType.error("JSC_T", "bad {0}");
    for (ErrorFormat fmt : new ErrorFormat[] {ErrorFormat.SINGLELINE, ErrorFormat.FULL, ErrorFormat.MULTILINE}) {
      PrintStreamErrorManager m = new PrintStreamErrorManager(fmt.toFormatter(p, false), ps);
      m.report(CheckLevel.ERROR, JSError.make("a.js", 1, 8, t, "\uD800"));
      m.report(CheckLevel.WARNING, JSError.make("a.js", 2, 12, t, "q"));
      m.generateReport();
    }
    ps.flush();
    System.out.print(java.util.HexFormat.of().formatHex(out.toByteArray()));
  }
}
