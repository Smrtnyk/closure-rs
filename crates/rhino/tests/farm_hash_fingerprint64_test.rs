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

use closure_rhino::common_hash::farm_hash_fingerprint64::{fingerprint, hash_bytes};

// oracle: corpus-cache/colors/vectors.txt (ColorVectors.java)
#[test]
fn farm_hash_fingerprint64_matches_java() {
    let vectors: &[(usize, u64, u64)] = &[
        (0, 0, 0x9ae16a3b2f90404f),
        (0, 1, 0x9ae16a3b2f90404f),
        (1, 0, 0x57821efdee1b7472),
        (1, 1, 0x3296282ee8c5eaca),
        (2, 0, 0x0265679dff512f39),
        (2, 1, 0x65e9030eb7e44406),
        (3, 0, 0x5f043801775be181),
        (3, 1, 0x15f5f13de09c03ee),
        (4, 0, 0xdcf15d9b38b260fc),
        (4, 1, 0xf2b7f2ff54b9c207),
        (5, 0, 0xc29967beedaa8a55),
        (5, 1, 0xa42eb53f0ea3d41e),
        (6, 0, 0x20521f96dd0d6554),
        (6, 1, 0x29e36e3c05c70600),
        (7, 0, 0x545ac134b57925ff),
        (7, 1, 0x4227b650c7b4ca80),
        (8, 0, 0xd14693440d28f69a),
        (8, 1, 0x0c2789eb49ab48f4),
        (9, 0, 0xc9d4703fa3ced177),
        (9, 1, 0xd8a48cba85e1754f),
        (10, 0, 0x92339a3db74778bf),
        (10, 1, 0xcfafc802416715da),
        (11, 0, 0x9545453e9ebd01b8),
        (11, 1, 0xcb012daf3b875298),
        (12, 0, 0xbdcdedef38b4a04e),
        (12, 1, 0x7fbbfdeaeeec3777),
        (13, 0, 0x20cb92a4c3d7d06a),
        (13, 1, 0xa2d9584b7b4627a8),
        (14, 0, 0x956a956c32faec12),
        (14, 1, 0x19a85a06cc978314),
        (15, 0, 0xc338033dfa6dca8d),
        (15, 1, 0x1340546e1bb0dea2),
        (16, 0, 0xd9b28ec31be83978),
        (16, 1, 0xf27d1c19cb6ef8f7),
        (17, 0, 0xffb7cd799a150d69),
        (17, 1, 0xd6083cc44da964a4),
        (18, 0, 0x555af2874a8a5747),
        (18, 1, 0xf5a9ae0dda78e462),
        (19, 0, 0xb8898bf2d6cacb32),
        (19, 1, 0x513511c025adcce4),
        (20, 0, 0xd458bee951a2bdc0),
        (20, 1, 0x9134ec73b017f68b),
        (21, 0, 0xda9abfc3aeac4bd5),
        (21, 1, 0xebb2b47ad8986d61),
        (22, 0, 0x32021e6ea9c6c42d),
        (22, 1, 0x9a0a507ad0b04e15),
        (23, 0, 0xb008a556a9fd9246),
        (23, 1, 0xb00361478cb039ff),
        (24, 0, 0x544ca270755e17e9),
        (24, 1, 0x20ca7169cb7c3a31),
        (25, 0, 0x6b1b5af22465ee4c),
        (25, 1, 0x9f15db49ddd2e4da),
        (26, 0, 0xd90802344edddf35),
        (26, 1, 0xa015b571949beb68),
        (27, 0, 0xbf8e069678f3cd9d),
        (27, 1, 0xebc2035359fdadb1),
        (28, 0, 0x5ba0325ae8d544f6),
        (28, 1, 0x36aefacdd861fed6),
        (29, 0, 0xe4e0ec7728ba2602),
        (29, 1, 0x6d68885dc9b6d2f4),
        (30, 0, 0x41cdcd3f47bbe07f),
        (30, 1, 0x3a0fc4a23c2cb6a4),
        (31, 0, 0x61a0625587879a75),
        (31, 1, 0x7ed504346a0b43a0),
        (32, 0, 0x5b0213e62a3ca399),
        (32, 1, 0xf9bfd0d88cb1e7dd),
        (33, 0, 0xd73bbb576c6c9808),
        (33, 1, 0xc0112342bbae196d),
        (34, 0, 0x95979238c8fba09d),
        (34, 1, 0xda28e69e0f634bd4),
        (35, 0, 0x935df6c4f798422d),
        (35, 1, 0xc5b34e3821273ae5),
        (36, 0, 0x991c4f8e0d57f9cd),
        (36, 1, 0x7529e4fd1569bedd),
        (37, 0, 0x590ee4b8831cd6ba),
        (37, 1, 0x148a69e159001029),
        (38, 0, 0x05f9f7bf0bb08f8a),
        (38, 1, 0xffdcff57d57736c3),
        (39, 0, 0xf10019ef658bdab7),
        (39, 1, 0xee1c0e7086342409),
        (40, 0, 0x968e3753320cf7f7),
        (40, 1, 0x3d1c75e73d20c094),
        (41, 0, 0xb484a1cc682b60b3),
        (41, 1, 0x75f82899bf6d8196),
        (42, 0, 0x72e555e5ce830082),
        (42, 1, 0x800515a405a74bc9),
        (43, 0, 0x880b6182d508e4c3),
        (43, 1, 0x0acedcb51c4418bb),
        (44, 0, 0x82cf6e8a14dcec32),
        (44, 1, 0x35a1de535284f7a4),
        (45, 0, 0x273289c73051002f),
        (45, 1, 0xfa388aac1f99053d),
        (46, 0, 0x08371506e4b7a7aa),
        (46, 1, 0xf07b89034ca6313f),
        (47, 0, 0x4e317b899409ea2d),
        (47, 1, 0x540fa5c0ffa518cb),
        (48, 0, 0x56558e9cb1196ff2),
        (48, 1, 0x4ddda55a6a39f53e),
        (49, 0, 0xbb547da3ab0c6ecd),
        (49, 1, 0x87fbbee8b3920b64),
        (50, 0, 0x5c7273cb9de6db5d),
        (50, 1, 0x522a6aad004a8bba),
        (51, 0, 0x66fb8e7503e07779),
        (51, 1, 0x16f4ede576bf5dd7),
        (52, 0, 0x85381ea1bd82bdba),
        (52, 1, 0x3d852f7a5f626a05),
        (53, 0, 0x9990975cca82949f),
        (53, 1, 0x0045394a5c8890d8),
        (54, 0, 0x34f75f2bbd17d060),
        (54, 1, 0xcebf68726eff75ba),
        (55, 0, 0xa3528e55ff855f69),
        (55, 1, 0xccbc6c7ab00babce),
        (56, 0, 0x1627a465945dce28),
        (56, 1, 0x2ecf6f7784dee928),
        (57, 0, 0xdd52b4eac65373f7),
        (57, 1, 0x4197316cf9b0a23f),
        (58, 0, 0xae0b7658335d7878),
        (58, 1, 0x5e1ec4b665325f84),
        (59, 0, 0x2cf9ea1b18a16b1e),
        (59, 1, 0x53fd8e325df78db4),
        (60, 0, 0x3ba329ec52a347ac),
        (60, 1, 0xea13c42be4d8d62e),
        (61, 0, 0xbea05dcf4c4d80ce),
        (61, 1, 0x01f875ade314fe39),
        (62, 0, 0x8aa438d498144eac),
        (62, 1, 0x12565fcceecc21c6),
        (63, 0, 0xff2033b26af5e749),
        (63, 1, 0x77272952ee974735),
        (64, 0, 0x56317ec658bfa4f9),
        (64, 1, 0x4309c1214b733ea9),
        (65, 0, 0x5a5b9a92eb3061d6),
        (65, 1, 0xb41584be02470af6),
        (66, 0, 0x1ca0813850a8dec3),
        (66, 1, 0x37303f417ca0fdd6),
        (67, 0, 0x8c8dd94245743c60),
        (67, 1, 0xe5cf0fb741c371fd),
        (68, 0, 0xf2affbdee5bdef53),
        (68, 1, 0xbc2eff6c55b81160),
        (69, 0, 0xad15d3b87682dc7a),
        (69, 1, 0x342ce1b73866f30b),
        (70, 0, 0xfd883728ad0e3b18),
        (70, 1, 0xbc76b818bd313037),
        (71, 0, 0x1623002a74e9a983),
        (71, 1, 0x26e837d13c671ba0),
        (72, 0, 0xaaa6c03efb141a0f),
        (72, 1, 0x35084c1573f7f73b),
        (73, 0, 0x9db5bb7b6302247b),
        (73, 1, 0x73d0fd2b28af5a31),
        (74, 0, 0x006d8052a1136475),
        (74, 1, 0xc05112d3485ab4df),
        (75, 0, 0x08e21dbc86485afd),
        (75, 1, 0x5f3d5cbe34f022f9),
        (76, 0, 0x51e6cedaf732a76a),
        (76, 1, 0x76e481ada37e67a0),
        (77, 0, 0x1e9130b6fbd0a372),
        (77, 1, 0xbe46c3cdc9f1a008),
        (78, 0, 0x518e6bb9af05b1f8),
        (78, 1, 0xc40f2803bfd6a75d),
        (79, 0, 0x22c87059186485fc),
        (79, 1, 0xe53ea3af4509eda4),
        (80, 0, 0x994b8eca31c3b6ae),
        (80, 1, 0x47e84a0d4ebbc84a),
        (96, 0, 0x45c4588c7dcc4030),
        (96, 1, 0x151fb40f0d0c28ae),
        (128, 0, 0x62d2c1879bd73de5),
        (128, 1, 0xaad8c6e7ae70ca7a),
        (129, 0, 0xd8e52f1caf21aae2),
        (129, 1, 0x74e54642c068b242),
        (200, 0, 0x8f2dac6e1bb7686d),
        (200, 1, 0xd683b5ef29a5b5ea),
        (255, 0, 0x15db64df7078c678),
        (255, 1, 0xee5a6d8e5bb335e3),
        (256, 0, 0x332b7d70dd660cb5),
        (256, 1, 0x4a9c02e87234e9b0),
        (300, 0, 0xdbe603e041287543),
        (300, 1, 0xa4abff9dfb540fc8),
    ];
    for &(length, pattern, expected) in vectors {
        let mut state = 0x9e3779b97f4a7c15u64
            .wrapping_mul(length as u64 + 1)
            .wrapping_add(pattern);
        let bytes: Vec<u8> = (0..length)
            .map(|i| {
                if pattern == 0 {
                    (i * 31 + 7) as u8
                } else {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                }
            })
            .collect();
        assert_eq!(
            hash_bytes(&bytes) as u64,
            expected,
            "length={length}, pattern={pattern}"
        );
        let mut padded = vec![0x99; 7];
        padded.extend_from_slice(&bytes);
        padded.extend_from_slice(&[0x99; 9]);
        assert_eq!(
            fingerprint(&padded, 7, length),
            expected,
            "offset=7, length={length}, pattern={pattern}"
        );
    }
}
