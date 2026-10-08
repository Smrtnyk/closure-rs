package com.google.javascript.jscomp;
import com.google.gson.Gson;
import com.google.javascript.rhino.IR;
import com.google.javascript.rhino.Node;
import java.util.*;
public final class CompilerEstimatorFixtures {
  public static void main(String[] args) {
    List<Map<String,Object>> rows=new ArrayList<>();
    for (String code: List.of("", "var a=1;", "function f(x){return x+1} f(2);", "const π=['hi','☺'];", "let s='\\uD800';", "class C{m(){return this.x}}", "var arr=[1,2,3,4,5,6,7,8,9];".repeat(20))) {
      Compiler compiler=new Compiler();
      Node root=IR.root(compiler.parseTestCode(code));
      PerformanceTrackerCodeSizeEstimator estimator=PerformanceTrackerCodeSizeEstimator.estimate(root,true);
      Map<String,Object> row=new LinkedHashMap<>();row.put("source",code);row.put("size",estimator.getCodeSize());row.put("gzip",estimator.getZippedCodeSize());rows.add(row);
    }
    System.out.println(new Gson().toJson(rows));
  }
}
