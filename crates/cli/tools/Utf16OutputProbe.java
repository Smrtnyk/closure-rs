package com.google.javascript.jscomp;
import java.io.*;
import java.nio.charset.*;
import java.util.HexFormat;
public class Utf16OutputProbe extends CommandLineRunner {
  Utf16OutputProbe() { super(new String[]{}, new ByteArrayInputStream(new byte[0]), System.out, System.err); }
  public static void main(String[] args) { try { probe(); } catch (Exception e) { throw new RuntimeException(e); } }
  static void probe() throws Exception {
    Utf16OutputProbe runner = new Utf16OutputProbe();
    String code = "A\ud800B\udc00C\ud83d\ude00";
    StringBuilder builder = new StringBuilder();
    runner.writeOutput(builder, null, code, "[%output%]", "%output%", null, "out.js");
    for (int i=0;i<builder.length();i++) System.out.printf("%04x ", (int)builder.charAt(i));
    System.out.println();
    for (String charset : new String[]{"UTF-8", "UTF-16BE", "UTF-16LE", "UTF-16", "US-ASCII", "ISO-8859-1"}) {
      ByteArrayOutputStream bytes = new ByteArrayOutputStream();
      Writer writer = new BufferedWriter(new OutputStreamWriter(bytes, Charset.forName(charset)));
      runner.writeOutput(writer, null, code, "[%output%]", "%output%", null, "out.js"); writer.close();
      System.out.println(charset+" "+HexFormat.of().formatHex(bytes.toByteArray()));
    }
    StringBuilder escaped = new StringBuilder();
    runner.writeOutput(escaped,null,code,"[%output%]","%output%",runner.getJavascriptEscaper(),"out.js");
    System.out.println(escaped.toString().replace("\n", "\\n"));
  }
}
