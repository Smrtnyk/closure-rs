package com.google.javascript.jscomp;
import com.google.gson.*;
import java.io.*;
import java.nio.*;
import java.nio.charset.*;
import java.nio.file.*;
import java.util.*;
import java.util.zip.*;
public final class SourceFileIoCases {
  static List<Integer> units(String s){List<Integer> r=new ArrayList<>();for(int i=0;i<s.length();i++)r.add((int)s.charAt(i));return r;}
  public static void main(String[] args)throws Exception{
    List<Object> cases=new ArrayList<>();List<byte[]> bytes=new ArrayList<>();
    bytes.add(new byte[0]);for(int b=0;b<256;b++)bytes.add(new byte[]{(byte)b});
    for(int[] b:new int[][]{{0xed,0xa0,0x80},{0xed,0xbf,0xbf},{0xe0,0x80,0x80},{0xf0,0x80,0x80,0x80},{0xf4,0x90,0x80,0x80},{0xe2,0x82},{0xe2,0x41,0x80},{0xc2,0x80},{0xf0,0x9f,0x98,0x80},{0xfe,0xff,0,0x41},{0xff,0xfe,0x41,0},{0xd8,0,0,0x41},{0,0xd8,0x41,0},{0xd8,0,0xdc,0},{0xdc,0},{0xd8,0,0x41},{0,0x41,0}}){byte[] bb=new byte[b.length];for(int i=0;i<b.length;i++)bb[i]=(byte)b[i];bytes.add(bb);}
    Random random=new Random(20261007);for(int n=0;n<200;n++){byte[] b=new byte[n%12];random.nextBytes(b);bytes.add(b);}
    for(String name:new String[]{"UTF-8","US-ASCII","ISO-8859-1","UTF-16","UTF-16BE","UTF-16LE"})for(byte[] b:bytes)for(boolean replace:new boolean[]{false,true}){
      Map<String,Object> row=new LinkedHashMap<>();row.put("charset",name);List<Integer> raw=new ArrayList<>();for(byte v:b)raw.add(v&255);row.put("bytes",raw);row.put("replace",replace);
      CharsetDecoder decoder=Charset.forName(name).newDecoder();if(replace)decoder.onMalformedInput(CodingErrorAction.REPLACE).onUnmappableCharacter(CodingErrorAction.REPLACE);
      try{row.put("units",units(decoder.decode(ByteBuffer.wrap(b)).toString()));}catch(CharacterCodingException e){row.put("exception",e.getMessage());}cases.add(row);
    }
    Path dir=Path.of(args[1]);Files.createDirectories(dir);Map<String,String> messages=new LinkedHashMap<>();
    Path missing=dir.resolve("missing.js");try{SourceFile.fromFile(missing.toString()).getCode();}catch(IOException e){messages.put("missing",e.getMessage());}
    Path invalid=dir.resolve("invalid.js");Files.write(invalid,new byte[]{(byte)0xff});try{SourceFile.fromFile(invalid.toString()).getCode();}catch(IOException e){messages.put("malformed",e.getMessage());}
    Path zip=dir.resolve("entry.zip");try(ZipOutputStream z=new ZipOutputStream(Files.newOutputStream(zip))){z.putNextEntry(new ZipEntry("yes.js"));z.write(65);z.closeEntry();}try{SourceFile.builder().withZipEntryPath(zip.toString(),"missing.js").build().getCode();}catch(NullPointerException e){messages.put("zip_missing",e.getMessage());}
    Files.writeString(Path.of(args[0]),new Gson().toJson(Map.of("cases",cases,"messages",messages)));System.out.println("charset rows="+cases.size()+" IO messages="+messages);
  }
}
