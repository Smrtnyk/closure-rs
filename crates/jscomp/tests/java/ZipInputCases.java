package com.google.javascript.jscomp;
import com.google.gson.*;
import java.io.*;
import java.nio.charset.*;
import java.nio.file.*;
import java.util.*;
import java.util.zip.*;
public final class ZipInputCases {
  static byte[] archive(boolean stored) throws Exception {ByteArrayOutputStream b=new ByteArrayOutputStream();try(ZipOutputStream z=new ZipOutputStream(b)){for(String name:new String[]{"foo.js","skip.txt","é😀.js","foo.js.map"}){byte[] data="// contents\n".getBytes(StandardCharsets.UTF_8);ZipEntry e=new ZipEntry(name);if(stored){CRC32 crc=new CRC32();crc.update(data);e.setMethod(0);e.setSize(data.length);e.setCrc(crc.getValue());}z.putNextEntry(e);z.write(data);z.closeEntry();}}return b.toByteArray();}
  static List<Object> row(byte[] b,String charset)throws Exception {List<Integer> bytes=new ArrayList<>();for(byte value:b)bytes.add(value&255);List<Object> result=new ArrayList<>();result.add(bytes);result.add(charset);try{List<String> names=new ArrayList<>();for(SourceFile f:SourceFile.fromZipInput("virtual.zip",new ByteArrayInputStream(b),Charset.forName(charset)))names.add(f.getName());result.add(Map.of("names",names));}catch(Exception e){Map<String,Object> failure=new LinkedHashMap<>();failure.put("exception",e.getMessage());result.add(failure);}return result;}
  public static void main(String[] args)throws Exception {
    List<Object> rows=new ArrayList<>();for(int n:new int[]{0,1,4,29,30,100})rows.add(row(new byte[n],"UTF-8"));
    for(boolean stored:new boolean[]{true,false}){byte[] full=archive(stored);rows.add(row(full,"UTF-8"));rows.add(row(full,"ISO-8859-1"));int central=0;for(int i=0;i<full.length-3;i++)if(full[i]==80&&full[i+1]==75&&full[i+2]==1&&full[i+3]==2){central=i;break;}rows.add(row(Arrays.copyOf(full,central),"UTF-8"));
      byte[] bad=full.clone();bad[6]|=1;rows.add(row(bad,"UTF-8"));bad=full.clone();bad[8]=99;rows.add(row(bad,"UTF-8"));bad=full.clone();bad[14]^=1;rows.add(row(bad,"UTF-8"));
      if(stored){bad=full.clone();bad[6]|=8;rows.add(row(bad,"UTF-8"));}
    }
    Files.writeString(Path.of(args[0]),new GsonBuilder().disableHtmlEscaping().serializeNulls().create().toJson(rows));
    List<Integer> zeros=new ArrayList<>();for(int c=0;c<=65535;c++)if(Character.digit((char)c,10)==0)zeros.add(c);System.out.println("BMP decimal zeros: "+zeros);System.out.println("zip stream rows="+rows.size());
  }
}
