// Writes crates/parsing/tests/fixtures/parser_runner_config.txt from the reference jar: the
// annotation names and suppression names of ParserRunner.createConfig(ECMASCRIPT3, no extra
// annotations, SLOPPY) and ParserRunner.getReservedVars(), each joined with ','.
// The resources step of scripts/upstream_sync.py runs it:
//   javac -cp closure-compiler.jar -d CLASSES GenParserRunnerConfig.java
//   java -cp closure-compiler.jar:CLASSES GenParserRunnerConfig OUT_FILE
import com.google.common.collect.ImmutableSet;
import com.google.javascript.jscomp.parsing.Config;
import com.google.javascript.jscomp.parsing.ParserRunner;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;

public class GenParserRunnerConfig {
  public static void main(String[] args) throws Exception {
    Config config =
        ParserRunner.createConfig(
            Config.LanguageMode.ECMASCRIPT3, ImmutableSet.of(), Config.StrictMode.SLOPPY);
    String text =
        "annotations=" + String.join(",", config.annotations().keySet()) + "\n"
            + "suppressions=" + String.join(",", config.suppressionNames()) + "\n"
            + "reserved=" + String.join(",", ParserRunner.getReservedVars()) + "\n";
    Files.write(Paths.get(args[0]), text.getBytes(StandardCharsets.UTF_8));
  }
}
