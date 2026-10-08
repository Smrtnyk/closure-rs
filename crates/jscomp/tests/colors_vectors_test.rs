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

use closure_jscomp::colors::{Color, ColorId, color_registry::REQUIRED_IDS, standard_colors as sc};
use closure_rhino::js_string::JsString;
use indexmap::{IndexMap, IndexSet};

// oracle: corpus-cache/colors/vectors.txt (ColorVectors.java)
#[test]
fn color_id_union_matches_java() {
    let vectors = r#"union 0x0000000000000061,0x0000000000000062, 26ddd2f1a7906d6b -1483707029
union 0x0000000000000061,0x0000000000000062,0x0000000000000063, 8690dc6a955ed8da -1788946214
union 0x0000000000000063,0x0000000000000062,0x0000000000000061, 8690dc6a955ed8da -1788946214
union 0x0000000000000061,0x0000000000000062,0x0000000000000000, a91f381e38af2e99 951004825
union 0x0000000000000061,0x0000000000000062,0x0000000000000001, 71c28d20e25072a8 -498044248
union 0xffffffffffffffff,0x0000000000000000,0x0000000000000001, 686d96fac3444a4a -1018934710
union 0x8000000000000000,0x7fffffffffffffff, 15596510e35346b7 -481081673
union 0x00000000d081722c,0x000000008c4d8f65, b1ab07907f193a31 2132359729
union 0x000000008c4d8f65,0x00000000d081722c, b1ab07907f193a31 2132359729
union 0x00000000d081722c,0x000000008c4d8f65,0x0000000022b49f69, 8068b9e5f1f8b57a -235358854
union 0x00000000234eb61a,0x00000000126812ee,0x0000000022b49f69,0x00000000d081722c,0x000000008c4d8f65,0x00000000759f2066,0x00000000889b6838,0x0000000000000000,0x00000000aaae3678,0x00000000283b5838, 2056553ef94c367d -112445827
union 0x000000001939a66d,0x0000000079d4a603,0x000000005627ffb0,0x000000009bb1303f,0x0000000046ab3f0e,0x00000000417ed2ab,0x00000000cb382e0a,0x0000000039581abf,0x00000000a9d9ad6d,0x000000009205dc06,0x0000000034ba2fb1,0x00000000186008a9,0x000000005e514f7e, 4ccfad36720d8ace 1913490126
union 0xfffffffffffffffb,0xfffffffffffffffc,0xfffffffffffffffd,0xfffffffffffffffe,0xffffffffffffffff,0x0000000000000000,0x0000000000000001,0x0000000000000002,0x0000000000000003,0x0000000000000004,0x0000000000000005,0x0000000000000006,0x0000000000000007,0x0000000000000008,0x0000000000000009, 32cd0d07d1d1fddf -774767137
unionk 2 8bf39bf243328c8f 1127386255
unionk 3 12ec71593d6cf658 1030551128
unionk 4 d2b6e31605f6ed60 100068704
unionk 5 5555d789696403d9 1768162265
unionk 6 b594a865db55ab57 -615142569
unionk 7 b8aee106eaaf299a -357619302
unionk 8 6d6a4b12264b4470 642466928
unionk 9 7ff632eeeaaddf6a -357703830
unionk 10 aca98968ca01b473 -905857933
unionk 11 38d38e9362f1f2f8 1660023544
unionk 12 2cb643829a57727c -1705545092
unionk 13 436f85266e0288db 1845659867
unionk 14 f1bacd9fb1501c33 -1320149965
unionk 15 5b731bca3f3c454f 1060914511
unionk 16 6e48fb26650dd09 1716575497
unionk 17 e82bb2890f45c569 256230761
unionk 18 ca057e66d98fc70c -644888820
unionk 19 4c00b8bf5cbfef9d 1556082589
unionk 20 c76b6e2b6806eee -1233096978
unionk 21 ba9f422457b9fb79 1471806329
unionk 22 609b0c5913d42d90 332672400
unionk 23 3efecd01c9763c6f -914998161
unionk 24 4cd7b14cb434e7e -884781442
unionk 25 d0349a0b0462b4f0 73577712
unionk 26 c1fc2098a0e9d6a4 -1595287900
unionk 27 16eabe39f2b6be58 -222904744
unionk 28 a86a9b33fce34bd5 -52212779
unionk 29 ff01ca63bfa33c56 -1079821226
unionk 30 7e7bdc96f58bad61 -175395487
unionk 31 7a88eabb5f58ba46 1599650374
unionk 32 fbf9419b4d90528 -1260845784
unionk 33 69d6817c016f974 -1072236172
unionk 34 644b67466e87433d 1854358333
unionk 35 e5957c2c1892a14f 412262735
unionk 36 e913676d6ab47dec 1790213612
unionk 37 c6c0d17c81172916 -2129188586
unionk 38 239f5ddfbc23c000 -1138507776
unionk 39 db0ebec87c4093ef 2084606959
unionk 40 8202f647dace176e -624027794"#;
    for line in vectors.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        let ids: IndexSet<ColorId> = if parts[0] == "union" {
            parts[1]
                .split(',')
                .filter(|x| !x.is_empty())
                .map(|x| ColorId::from_unsigned(u64::from_str_radix(&x[2..], 16).unwrap() as i64))
                .collect()
        } else {
            let k: i64 = parts[1].parse().unwrap();
            (1..=k)
                .rev()
                .map(|i| ColorId::from_unsigned(i * 0x0101010101))
                .collect()
        };
        let union = ColorId::union(&ids);
        assert_eq!(union.to_string(), parts[2], "{line}");
        assert_eq!(
            union.hash_code(),
            parts[3].parse::<i32>().unwrap(),
            "{line}"
        );
    }
}

// oracle: corpus-cache/colors/vectors.txt (ColorVectors.java)
#[allow(clippy::disallowed_names)]
fn vector_colors() -> IndexMap<&'static str, Color> {
    let foo = Color::single_builder()
        .set_id(ColorId::from_ascii("Foo"))
        .build();
    let foo_proto = Color::single_builder()
        .set_id(ColorId::from_ascii("FooP"))
        .set_own_properties(
            ["a", "b", "constructor"]
                .into_iter()
                .map(JsString::from)
                .collect(),
        )
        .build();
    let foo_ctor = Color::single_builder()
        .set_id(ColorId::from_ascii("FooC"))
        .set_constructor(true)
        .set_instance_color(Some(foo.clone()))
        .set_prototype(Some(foo_proto.clone()))
        .set_invalidating(true)
        .build();
    let bar = Color::single_builder()
        .set_id(ColorId::from_ascii("Bar"))
        .set_closure_assert(true)
        .set_properties_keep_original_name(true)
        .build();
    let num_or_str = Color::create_union(&IndexSet::from([sc::STRING.clone(), sc::NUMBER.clone()]));
    let big = Color::create_union(&IndexSet::from([
        num_or_str.clone(),
        sc::BIGINT.clone(),
        sc::NULL_OR_VOID.clone(),
    ]));
    let ctor_union = Color::create_union(&IndexSet::from([
        foo_ctor.clone(),
        bar.clone(),
        sc::NULL_OR_VOID.clone(),
    ]));
    IndexMap::from([
        ("BIGINT", sc::BIGINT.clone()),
        ("BOOLEAN", sc::BOOLEAN.clone()),
        ("NULL_OR_VOID", sc::NULL_OR_VOID.clone()),
        ("NUMBER", sc::NUMBER.clone()),
        ("STRING", sc::STRING.clone()),
        ("SYMBOL", sc::SYMBOL.clone()),
        ("TOP_OBJECT", sc::TOP_OBJECT.clone()),
        ("TOP_FUNCTION", sc::TOP_FUNCTION.clone()),
        ("UNKNOWN", sc::UNKNOWN.clone()),
        ("GBIGINT", sc::GBIGINT.clone()),
        ("foo", foo),
        ("fooProto", foo_proto),
        ("fooCtor", foo_ctor),
        ("bar", bar),
        ("numOrStr", num_or_str),
        ("big", big.clone()),
        ("big.subtractNullOrVoid", big.subtract_null_or_void()),
        ("ctorUnion", ctor_union.clone()),
        (
            "ctorUnion.subtractNullOrVoid",
            ctor_union.subtract_null_or_void(),
        ),
    ])
}

// oracle: corpus-cache/colors/vectors.txt (ColorVectors.java)
const COLOR_VECTORS: &str = r#"color BIGINT | Color{id=234eb61a, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=a9d9ad6d, closureAssert=false, unionElements=[]} | 539992198
color BOOLEAN | Color{id=126812ee, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=9205dc06, closureAssert=false, unionElements=[]} | -1006537463
color NULL_OR_VOID | Color{id=22b49f69, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]} | 934811842
color NUMBER | Color{id=d081722c, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=34ba2fb1, closureAssert=false, unionElements=[]} | -13123972
color STRING | Color{id=8c4d8f65, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=186008a9, closureAssert=false, unionElements=[]} | -401897621
color SYMBOL | Color{id=759f2066, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=5e514f7e, closureAssert=false, unionElements=[]} | -1811168711
color TOP_OBJECT | Color{id=889b6838, prototypes=[], instanceColors=[], invalidating=true, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]} | 394134671
color TOP_FUNCTION | Color{id=283b5838, prototypes=[], instanceColors=[], invalidating=true, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]} | -1244932977
color UNKNOWN | Color{id=0, prototypes=[], instanceColors=[], invalidating=true, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]} | 1053378423
color GBIGINT | Color{id=aaae3678, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]} | -132056187
color foo | Color{id=466f6f, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]} | 725370872
color fooProto | Color{id=466f6f50, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[a, b, constructor], boxId=null, closureAssert=false, unionElements=[]} | 1129658798
color fooCtor | Color{id=466f6f43, prototypes=[Color{id=466f6f50, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[a, b, constructor], boxId=null, closureAssert=false, unionElements=[]}], instanceColors=[Color{id=466f6f, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}], invalidating=true, propertiesKeepOriginalName=false, constructor=true, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]} | 1925718694
color bar | Color{id=426172, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=true, constructor=false, ownProperties=[], boxId=null, closureAssert=true, unionElements=[]} | 1992075843
color numOrStr | Color{id=b1ab07907f193a31, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=8c4d8f65, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=186008a9, closureAssert=false, unionElements=[]}, Color{id=d081722c, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=34ba2fb1, closureAssert=false, unionElements=[]}]} | -920430083
color big | Color{id=69e89bd9d35242cf, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=8c4d8f65, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=186008a9, closureAssert=false, unionElements=[]}, Color{id=d081722c, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=34ba2fb1, closureAssert=false, unionElements=[]}, Color{id=234eb61a, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=a9d9ad6d, closureAssert=false, unionElements=[]}, Color{id=22b49f69, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}]} | 1958729527
color big.subtractNullOrVoid | Color{id=47b1e8c6b7ad470c, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=8c4d8f65, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=186008a9, closureAssert=false, unionElements=[]}, Color{id=d081722c, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=34ba2fb1, closureAssert=false, unionElements=[]}, Color{id=234eb61a, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=a9d9ad6d, closureAssert=false, unionElements=[]}]} | -1582139236
color ctorUnion | Color{id=bc70c36ec87fc026, prototypes=[Color{id=466f6f50, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[a, b, constructor], boxId=null, closureAssert=false, unionElements=[]}], instanceColors=[Color{id=466f6f, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}], invalidating=true, propertiesKeepOriginalName=true, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=466f6f43, prototypes=[Color{id=466f6f50, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[a, b, constructor], boxId=null, closureAssert=false, unionElements=[]}], instanceColors=[Color{id=466f6f, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}], invalidating=true, propertiesKeepOriginalName=false, constructor=true, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}, Color{id=426172, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=true, constructor=false, ownProperties=[], boxId=null, closureAssert=true, unionElements=[]}, Color{id=22b49f69, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}]} | -1734879750
color ctorUnion.subtractNullOrVoid | Color{id=734c6fcc91d46116, prototypes=[Color{id=466f6f50, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[a, b, constructor], boxId=null, closureAssert=false, unionElements=[]}], instanceColors=[Color{id=466f6f, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}], invalidating=true, propertiesKeepOriginalName=true, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=466f6f43, prototypes=[Color{id=466f6f50, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[a, b, constructor], boxId=null, closureAssert=false, unionElements=[]}], instanceColors=[Color{id=466f6f, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}], invalidating=true, propertiesKeepOriginalName=false, constructor=true, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}, Color{id=426172, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=true, constructor=false, ownProperties=[], boxId=null, closureAssert=true, unionElements=[]}]} | 211250856"#;

// oracle: corpus-cache/colors/vectors.txt (ColorVectors.java)
#[test]
fn color_hash_code_matches_java() {
    let colors = vector_colors();
    for line in COLOR_VECTORS.lines() {
        let parts: Vec<_> = line.split(" | ").collect();
        let name = parts[0].strip_prefix("color ").unwrap();
        assert_eq!(
            colors[name].hash_code(),
            parts[2].parse::<i32>().unwrap(),
            "{name}"
        );
    }
}

// oracle: corpus-cache/colors/vectors.txt (ColorVectors.java)
#[test]
fn color_display_matches_java() {
    let colors = vector_colors();
    for line in COLOR_VECTORS.lines() {
        let parts: Vec<_> = line.split(" | ").collect();
        let name = parts[0].strip_prefix("color ").unwrap();
        assert_eq!(colors[name].to_string(), parts[1], "{name}");
    }
}

// oracle: corpus-cache/colors/vectors.txt (ColorVectors.java)
#[test]
fn standard_color_id_order_and_hash_code_match_java() {
    let vectors = r#"box a9d9ad6d -1445352083
box 9205dc06 -1845109754
box 34ba2fb1 884617137
box 186008a9 408946857
box 5e514f7e 1582387070
required a9d9ad6d -1445352083
required 9205dc06 -1845109754
required 34ba2fb1 884617137
required 186008a9 408946857
required 5e514f7e 1582387070
required 79d4a603 2043979267
required 5627ffb0 1445461936
required 1939a66d 423208557
required cb382e0a -885510646
required 9bb1303f -1682886593
required 46ab3f0e 1185627918
required 417ed2ab 1098830507
required 39581abf 962075327
axiomatic 234eb61a 592360986
axiomatic 126812ee 308810478
axiomatic 22b49f69 582262633
axiomatic d081722c -796822996
axiomatic 8c4d8f65 -1941074075
axiomatic 759f2066 1973362790
axiomatic 889b6838 -2003081160
axiomatic 0 0
axiomatic aaae3678 -1431423368
axiomatic 283b5838 674977848
primitive 234eb61a 592360986
primitive 126812ee 308810478
primitive 22b49f69 582262633
primitive d081722c -796822996
primitive 8c4d8f65 -1941074075
primitive 759f2066 1973362790
primitive aaae3678 -1431423368"#;
    let mut actual = Vec::new();
    for id in sc::PRIMITIVE_BOX_IDS.iter() {
        actual.push(format!("box {id} {}", id.hash_code()));
    }
    for id in REQUIRED_IDS.iter() {
        actual.push(format!("required {id} {}", id.hash_code()));
    }
    for id in sc::AXIOMATIC_COLORS.keys() {
        actual.push(format!("axiomatic {id} {}", id.hash_code()));
    }
    for id in sc::PRIMITIVE_COLORS.keys() {
        actual.push(format!("primitive {id} {}", id.hash_code()));
    }
    assert_eq!(actual.join("\n"), vectors);
}
