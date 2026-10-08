package com.google.javascript.jscomp;
public final class CompilerTracerReport {
  public static void main(String[] args) {
    Tracer.clock = new Tracer.InternalClock() {
      private long time = 1000;
      public long currentTimeMillis() {long result=time; time+=5; return result;}
    };
    Tracer.setPrettyPrint(true);
    Tracer.initCurrentThreadTrace(0);
    Tracer outer=new Tracer("A", "outer");
    Tracer inner=new Tracer("A", "inner");
    inner.stop();outer.stop();
    Tracer silent=new Tracer("A", "silent");silent.stop(10);
    System.out.print(Tracer.getCurrentThreadTraceReport());
  }
}
