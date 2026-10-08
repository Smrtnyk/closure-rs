/*
 * Copyright 2026 The closure-rs Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Checks of `XtbMessageBundle` (and the JDK SAX parser behaviour it relies on) against results
//! recorded with the Java implementation on JDK 21 (`XtbProbe2`, kept with its inputs in
//! corpus-cache/messages/xtbprobe): the message parts, i.e. how Xerces splits character data
//! into `characters()` calls, and the `SAXParseException` texts of malformed bundles. Not a port
//! of a Java test class.

use closure_jscomp::message_bundle::MessageBundle;
use closure_jscomp::xtb_message_bundle::XtbMessageBundle;

fn esc(units: &[u16]) -> String {
    let mut sb = String::new();
    for &c in units {
        match c {
            0x0A => sb.push_str("\\n"),
            0x0D => sb.push_str("\\r"),
            0x09 => sb.push_str("\\t"),
            c if !(0x20..=0x7e).contains(&c) => sb.push_str(&format!("\\u{c:04x}")),
            c => sb.push(c as u8 as char),
        }
    }
    sb
}

/// The probe's rendering of a bundle: `id:[part][PH NAME]...` per message, or the exception.
fn render(bytes: &[u8]) -> String {
    match XtbMessageBundle::new(bytes, Some("P")) {
        Ok(bundle) => {
            let mut out = Vec::new();
            for m in bundle.get_all_messages() {
                let mut sb = format!("{}:", m.get_id().to_string_lossy());
                for p in m.get_parts() {
                    if p.is_placeholder() {
                        sb.push_str(&format!(
                            "[PH {}]",
                            p.get_canonical_placeholder_name().to_string_lossy()
                        ));
                    } else {
                        sb.push_str(&format!("[{}]", esc(p.get_string().as_units())));
                    }
                }
                out.push(sb);
            }
            out.join(" ")
        }
        Err(e) => format!("ERROR java.lang.RuntimeException: {e}"),
    }
}

const CASES: &[(&str, &[u8], &str)] = &[
    (
        "c01.xtb",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n<translationbundle lang=\"en\">\n<translation id=\"1\">a\nb\nc\nd\ne</translation><translation id=\"2\">a\n\nb</translation><translation id=\"3\">\nlead</translation><translation id=\"4\">a &lt; b &amp; c &#65;d&#x42;e</translation><translation id=\"5\">x<![CDATA[y<z]]>w</translation><translation id=\"6\">a]b]]c</translation><translation id=\"7\">&quot;q&apos; &gt;</translation></translationbundle>\n",
        "1:[a\\nb][\\nc\\nd][\\ne] 2:[a\\n\\nb] 3:[\\nlead] 4:[a ][<][ b ][&][ c ][A][d][B][e] 5:[x][y<z][w] 6:[a]b]]][c] 7:[\"][q]['][ ][>]",
    ),
    (
        "c02.xtb",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n<translationbundle lang=\"en\">\n<translation id=\"8\">a\xf0\x9f\x98\x80b\xf0\x9f\x98\x80</translation><translation id=\"9\">a\r\nb\rc\r\n\r\nd</translation><translation id=\"10\">x&#13;y</translation><translation id=\"11\">&nbsp;x&nbsp;</translation><translation id=\"12\"><![CDATA[]]></translation><translation id=\"13\">a<!-- c -->b<?pi d?>c</translation></translationbundle>",
        "8:[a\\ud83d\\ude00][b\\ud83d\\ude00] 9:[a\\nb][\\nc\\n\\nd] 10:[x][\\r][y] 11:[x] 12:[] 13:[a][b][c]",
    ),
    (
        "c03.xtb",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n<translationbundle lang=\"en\">\n<translation id=\"14\">line1\n  <ph name=\"A\"/>\n  line2</translation><translation id=\"15\"><ph name=\"A\"/>\n</translation><translation id=\"16\">\xc3\xa9\xc2\xa0\xc3\xbc</translation><translation id=\"17\">]]x</translation><translation id=\"18\">\n\n</translation><translation id=\"19\">a\n</translation><translation id=\"20\">&#x1F600;&#10;</translation></translationbundle>",
        "14:[line1\\n  ][PH A][\\n  line2] 15:[PH A][\\n] 16:[\\u00e9\\u00a0\\u00fc] 17:[]]x] 18:[\\n\\n] 19:[a\\n] 20:[\\ud83d\\ude00][\\n]",
    ),
    (
        "c04.xtb",
        b"<?xml version=\"1.0\"?>\n<!DOCTYPE translationbundle [\n<!ENTITY e \"abc\">\n<!ENTITY n \"x&#10;y\nz\">\n<!ENTITY p \"<ph name='Q'/>t\">\n<!ATTLIST translation id CDATA #REQUIRED>\n<!-- c -->\n]>\n<translationbundle lang=\"en\"><translation id=\"21\">1&e;2</translation><translation id=\"22\">&n;</translation><translation id=\"23\">&e;</translation><translation id=\"24\">a&p;b</translation><translation id=\"25\">x&e;\ny</translation></translationbundle>",
        "21:[1][abc2] 22:[x\\ny][\\nz] 23:[abc] 24:[a][PH Q][tb] 25:[x][abc\\ny]",
    ),
    (
        "e01.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">a&foo;b</translation></translationbundle>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 56; The entity \"foo\" was referenced, but not declared.",
    ),
    (
        "e02.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">ab</translation>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 66; XML document structures must start and end within the same entity.",
    ),
    (
        "e03.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">ab</b></translationbundle>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 54; The element type \"translation\" must be terminated by the matching end-tag \"</translation>\".",
    ),
    (
        "e04.xtb",
        b"junk<translationbundle lang=\"en\"/>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 1; Content is not allowed in prolog.",
    ),
    (
        "e05.xtb",
        b"",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 1; Premature end of file.",
    ),
    (
        "e06.xtb",
        b"<translationbundle lang=en/>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 25; Open quote is expected for attribute \"lang\" associated with an  element type  \"translationbundle\".",
    ),
    (
        "e07.xtb",
        b"<translationbundle lang=\"en\"/>junk",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 31; Content is not allowed in trailing section.",
    ),
    (
        "e08.xtb",
        b"<translationbundle lang=\"en\" lang=\"x\"/>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 38; Attribute \"lang\" was already specified for element \"translationbundle\".",
    ),
    (
        "e09.xtb",
        b"<translationbundle lang=\"e<n\"/>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 27; The value of attribute \"lang\" associated with an element type \"translationbundle\" must not contain the '<' character.",
    ),
    (
        "e10.xtb",
        b"<translationbundle lang=\"en\">\n<translation id=\"1\">a]]>b</translation></translationbundle>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 2; columnNumber: 25; The character sequence \"]]>\" must not appear in content unless used to mark the end of a CDATA section.",
    ),
    (
        "e11.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">a\xff</translation></translationbundle>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 50; Invalid byte 1 of 1-byte UTF-8 sequence.",
    ),
    (
        "e12.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">a\x01</translation></translationbundle>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 51; An invalid XML character (Unicode: 0x1) was found in the element content of the document.",
    ),
    (
        "e14.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">&#0;</translation></translationbundle>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 54; Character reference \"&#0\" is an invalid XML character.",
    ),
    (
        "e16.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">a & b</translation></translationbundle>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 1; columnNumber: 53; The entity name must immediately follow the '&' in the entity reference.",
    ),
    (
        "e17.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">a</translation >\n</translationbundle\n>\n<!-- x -->\n",
        "1:[a]",
    ),
    (
        "f01.xtb",
        b"<?xml version=\"1.0\" standalone=\"yes\"?>\n<!DOCTYPE translationbundle SYSTEM \"x.dtd\">\n<translationbundle lang=\"en\"><translation id=\"1\">&nbsp;</translation></translationbundle>",
        "ERROR java.lang.RuntimeException: org.xml.sax.SAXParseException; lineNumber: 3; columnNumber: 56; The entity \"nbsp\" was referenced, but not declared.",
    ),
    (
        "f02.xtb",
        b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<!DOCTYPE translationbundle PUBLIC \"-//X//Y\" \"x.dtd\">\n<?pi x?>\n<!-- c -->\n<translationbundle lang=\"en\"><translation id=\"1\">a<!--x-->b\n<?p?>c</translation></translationbundle>",
        "1:[a][b\\n][c]",
    ),
    (
        "f03.xtb",
        b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<translationbundle lang=\"en\"><translation id=\"1\">caf\xe9</translation></translationbundle>",
        "1:[caf\\u00e9]",
    ),
    (
        "f04.xtb",
        b"<!DOCTYPE translationbundle [<!ATTLIST translation id CDATA \"dflt\">]>\n<translationbundle lang=\"en\"><translation>x</translation></translationbundle>",
        "dflt:[x]",
    ),
    (
        "f06.xtb",
        b"<translationbundle lang=\"en\"><translation id=\"1\">a&#x0D;\nb</translation><translation id=\"2\">&lt;&#60;x</translation><translation id=\"3\">a&#93;]>b</translation></translationbundle>",
        "1:[a][\\r][\\nb] 2:[<][<][x] 3:[a][]][]>b]",
    ),
    (
        "g01.xtb",
        b"<!DOCTYPE translationbundle [<!ATTLIST translation id NMTOKEN #IMPLIED>]>\n<translationbundle lang=\"en\"><translation id=\"  a  \">x</translation></translationbundle>",
        "a:[x]",
    ),
    (
        "g02.xtb",
        b"<!DOCTYPE translationbundle [<!ATTLIST translation id CDATA \"d&#32;f\tl\"><!ATTLIST translation id CDATA \"second\"><!ENTITY e \"E\"><!ATTLIST ph name CDATA \"N&e;\">]>\n<translationbundle lang=\"en\"><translation>x<ph/></translation><translation id=\"b c\">y</translation></translationbundle>",
        "d f l:[x][PH NE] b c:[y]",
    ),
    (
        "g03.xtb",
        b"<!DOCTYPE translationbundle SYSTEM \"x.dtd\" [<!ATTLIST translation id CDATA #FIXED \"fx\" variants (a|b) \"a\">]>\n<translationbundle lang=\"en\"><translation>x</translation></translationbundle>",
        "fx:[x]",
    ),
    (
        "g04.xtb",
        b"<!DOCTYPE translationbundle [<!ATTLIST translation id NMTOKENS \"  p   q  \">]>\n<translationbundle lang=\"en\"><translation>x</translation><translation id=\" r   s \">x</translation></translationbundle>",
        "p q:[x] r s:[x]",
    ),
];

#[test]
fn parts_and_errors_match_java() {
    for (name, input, expected) in CASES {
        assert_eq!(&render(input), expected, "{name}");
    }
}
