/*
 * Copyright (C) 2015 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
 * in compliance with the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License
 * is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
 * or implied. See the License for the specific language governing permissions and limitations under
 * the License.
 */
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/hash/FarmHashFingerprint64.java.

//! Guava's FarmHashFingerprint64, with Java long overflow and little-endian loads.
const K0: u64 = 0xc3a5c85c97cb3127;
const K1: u64 = 0xb492b66fbe98f273;
const K2: u64 = 0x9ae16a3b2f90404f;

// port: FarmHashFingerprint64#hashBytes
pub fn hash_bytes(bytes: &[u8]) -> i64 {
    fingerprint(bytes, 0, bytes.len()) as i64
}

// port: FarmHashFingerprint64#fingerprint
pub fn fingerprint(bytes: &[u8], offset: usize, length: usize) -> u64 {
    if length <= 32 {
        if length <= 16 {
            hash_length0to16(bytes, offset, length)
        } else {
            hash_length17to32(bytes, offset, length)
        }
    } else if length <= 64 {
        hash_length33_to64(bytes, offset, length)
    } else {
        hash_length65_plus(bytes, offset, length)
    }
}

// port: FarmHashFingerprint64#load64
fn load64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

// port: FarmHashFingerprint64#load32
fn load32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

// port: FarmHashFingerprint64#rotateRight
fn rotate_right(value: u64, distance: u32) -> u64 {
    value.rotate_right(distance)
}

// port: FarmHashFingerprint64#shiftMix
fn shift_mix(value: u64) -> u64 {
    value ^ (value >> 47)
}

// port: FarmHashFingerprint64#hashLength16
fn hash_length16(u: u64, v: u64, mul: u64) -> u64 {
    let mut a = (u ^ v).wrapping_mul(mul);
    a ^= a >> 47;
    let mut b = (v ^ a).wrapping_mul(mul);
    b ^= b >> 47;
    b = b.wrapping_mul(mul);
    b
}

// port: FarmHashFingerprint64#weakHashLength32WithSeeds
fn weak_hash_length32_with_seeds(
    bytes: &[u8],
    offset: usize,
    mut seed_a: u64,
    mut seed_b: u64,
    output: &mut [u64; 2],
) {
    let part1 = load64(bytes, offset);
    let part2 = load64(bytes, offset + 8);
    let part3 = load64(bytes, offset + 16);
    let part4 = load64(bytes, offset + 24);
    seed_a = seed_a.wrapping_add(part1);
    seed_b = rotate_right(seed_b.wrapping_add(seed_a).wrapping_add(part4), 21);
    let c = seed_a;
    seed_a = seed_a.wrapping_add(part2);
    seed_a = seed_a.wrapping_add(part3);
    seed_b = seed_b.wrapping_add(rotate_right(seed_a, 44));
    output[0] = seed_a.wrapping_add(part4);
    output[1] = seed_b.wrapping_add(c);
}

// port: FarmHashFingerprint64#hashLength0to16
fn hash_length0to16(bytes: &[u8], offset: usize, length: usize) -> u64 {
    if length >= 8 {
        let mul = K2.wrapping_add((length as u64).wrapping_mul(2));
        let a = load64(bytes, offset).wrapping_add(K2);
        let b = load64(bytes, offset + length - 8);
        let c = rotate_right(b, 37).wrapping_mul(mul).wrapping_add(a);
        let d = rotate_right(a, 25).wrapping_add(b).wrapping_mul(mul);
        return hash_length16(c, d, mul);
    }
    if length >= 4 {
        let mul = K2.wrapping_add((length as u64).wrapping_mul(2));
        let a = load32(bytes, offset) as u64;
        return hash_length16(
            (length as u64).wrapping_add(a << 3),
            load32(bytes, offset + length - 4) as u64,
            mul,
        );
    }
    if length > 0 {
        let a = bytes[offset] as u32;
        let b = bytes[offset + (length >> 1)] as u32;
        let c = bytes[offset + length - 1] as u32;
        let y = a + (b << 8);
        let z = length as u32 + (c << 2);
        return shift_mix((y as u64).wrapping_mul(K2) ^ (z as u64).wrapping_mul(K0))
            .wrapping_mul(K2);
    }
    K2
}

// port: FarmHashFingerprint64#hashLength17to32
fn hash_length17to32(bytes: &[u8], offset: usize, length: usize) -> u64 {
    let mul = K2.wrapping_add((length as u64).wrapping_mul(2));
    let a = load64(bytes, offset).wrapping_mul(K1);
    let b = load64(bytes, offset + 8);
    let c = load64(bytes, offset + length - 8).wrapping_mul(mul);
    let d = load64(bytes, offset + length - 16).wrapping_mul(K2);
    hash_length16(
        rotate_right(a.wrapping_add(b), 43)
            .wrapping_add(rotate_right(c, 30))
            .wrapping_add(d),
        a.wrapping_add(rotate_right(b.wrapping_add(K2), 18))
            .wrapping_add(c),
        mul,
    )
}

// port: FarmHashFingerprint64#hashLength33To64
fn hash_length33_to64(bytes: &[u8], offset: usize, length: usize) -> u64 {
    let mul = K2.wrapping_add((length as u64).wrapping_mul(2));
    let a = load64(bytes, offset).wrapping_mul(K2);
    let b = load64(bytes, offset + 8);
    let c = load64(bytes, offset + length - 8).wrapping_mul(mul);
    let d = load64(bytes, offset + length - 16).wrapping_mul(K2);
    let y = rotate_right(a.wrapping_add(b), 43)
        .wrapping_add(rotate_right(c, 30))
        .wrapping_add(d);
    let z = hash_length16(
        y,
        a.wrapping_add(rotate_right(b.wrapping_add(K2), 18))
            .wrapping_add(c),
        mul,
    );
    let e = load64(bytes, offset + 16).wrapping_mul(mul);
    let f = load64(bytes, offset + 24);
    let g = y
        .wrapping_add(load64(bytes, offset + length - 32))
        .wrapping_mul(mul);
    let h = z
        .wrapping_add(load64(bytes, offset + length - 24))
        .wrapping_mul(mul);
    hash_length16(
        rotate_right(e.wrapping_add(f), 43)
            .wrapping_add(rotate_right(g, 30))
            .wrapping_add(h),
        e.wrapping_add(rotate_right(f.wrapping_add(a), 18))
            .wrapping_add(g),
        mul,
    )
}

// port: FarmHashFingerprint64#hashLength65Plus
fn hash_length65_plus(bytes: &[u8], mut offset: usize, length: usize) -> u64 {
    let seed = 81u64;
    let mut x = seed;
    let mut y = seed.wrapping_mul(K1).wrapping_add(113);
    let mut z = shift_mix(y.wrapping_mul(K2).wrapping_add(113)).wrapping_mul(K2);
    let mut v = [0u64; 2];
    let mut w = [0u64; 2];
    x = x.wrapping_mul(K2).wrapping_add(load64(bytes, offset));
    let end = offset + ((length - 1) / 64) * 64;
    let last64offset = end + ((length - 1) & 63) - 63;
    loop {
        x = rotate_right(
            x.wrapping_add(y)
                .wrapping_add(v[0])
                .wrapping_add(load64(bytes, offset + 8)),
            37,
        )
        .wrapping_mul(K1);
        y = rotate_right(
            y.wrapping_add(v[1])
                .wrapping_add(load64(bytes, offset + 48)),
            42,
        )
        .wrapping_mul(K1);
        x ^= w[1];
        y = y.wrapping_add(v[0].wrapping_add(load64(bytes, offset + 40)));
        z = rotate_right(z.wrapping_add(w[0]), 33).wrapping_mul(K1);
        weak_hash_length32_with_seeds(
            bytes,
            offset,
            v[1].wrapping_mul(K1),
            x.wrapping_add(w[0]),
            &mut v,
        );
        weak_hash_length32_with_seeds(
            bytes,
            offset + 32,
            z.wrapping_add(w[1]),
            y.wrapping_add(load64(bytes, offset + 16)),
            &mut w,
        );
        std::mem::swap(&mut x, &mut z);
        offset += 64;
        if offset == end {
            break;
        }
    }
    let mul = K1.wrapping_add((z & 0xff) << 1);
    offset = last64offset;
    w[0] = w[0].wrapping_add(((length - 1) & 63) as u64);
    v[0] = v[0].wrapping_add(w[0]);
    w[0] = w[0].wrapping_add(v[0]);
    x = rotate_right(
        x.wrapping_add(y)
            .wrapping_add(v[0])
            .wrapping_add(load64(bytes, offset + 8)),
        37,
    )
    .wrapping_mul(mul);
    y = rotate_right(
        y.wrapping_add(v[1])
            .wrapping_add(load64(bytes, offset + 48)),
        42,
    )
    .wrapping_mul(mul);
    x ^= w[1].wrapping_mul(9);
    y = y.wrapping_add(
        v[0].wrapping_mul(9)
            .wrapping_add(load64(bytes, offset + 40)),
    );
    z = rotate_right(z.wrapping_add(w[0]), 33).wrapping_mul(mul);
    weak_hash_length32_with_seeds(
        bytes,
        offset,
        v[1].wrapping_mul(mul),
        x.wrapping_add(w[0]),
        &mut v,
    );
    weak_hash_length32_with_seeds(
        bytes,
        offset + 32,
        z.wrapping_add(w[1]),
        y.wrapping_add(load64(bytes, offset + 16)),
        &mut w,
    );
    hash_length16(
        hash_length16(v[0], w[0], mul)
            .wrapping_add(shift_mix(y).wrapping_mul(K0))
            .wrapping_add(x),
        hash_length16(v[1], w[1], mul).wrapping_add(z),
        mul,
    )
}
