/*
 * Copyright 2011 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/regex/CharRanges.java.

//! Port of `com.google.javascript.jscomp.regex.CharRanges`.

use std::fmt;
use std::sync::LazyLock;

use closure_rhino::java_lang;

/// An immutable sparse bitset that deals well where the data is chunky: where P(bit[x+1] ==
/// bit[x]). E.g. [101,102,103,104,105,1001,1002,1003,1004] is chunky.
// port: CharRanges#equals (derived PartialEq: Arrays.equals on ranges)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CharRanges {
    /// A strictly increasing set of bit indices where even members are the inclusive starts of
    /// ranges, and odd members are the exclusive ends.
    ///
    /// E.g., { 1, 5, 6, 10 } represents the set ( 1, 2, 3, 4, 6, 7, 8, 9 ).
    ranges: Vec<i32>,
}

/// `CharRanges.EMPTY`
pub static EMPTY: LazyLock<CharRanges> = LazyLock::new(|| CharRanges::new(Vec::new()));

/// `CharRanges.ALL_CODE_UNITS`
pub static ALL_CODE_UNITS: LazyLock<CharRanges> =
    LazyLock::new(|| CharRanges::new(vec![0, 0x10000]));

/// `java.lang.IndexOutOfBoundsException` thrown by `CharRanges.inclusive` and `shift`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexOutOfBoundsException {
    pub message: Option<String>,
}

impl CharRanges {
    // port: CharRanges#inclusive
    pub fn inclusive(start: i32, end: i32) -> Result<CharRanges, IndexOutOfBoundsException> {
        if start > end {
            return Err(IndexOutOfBoundsException {
                message: Some(format!("{start} > {end}")),
            });
        }
        Ok(CharRanges::new(vec![start, end.wrapping_add(1)]))
    }

    /// Returns an instance containing all and only the given members.
    // port: CharRanges#withMembers
    pub fn with_members(members: &[i32]) -> CharRanges {
        CharRanges::new(Self::int_array_to_ranges(&mut members.to_vec()))
    }

    /// Returns an instance containing the given ranges.
    ///
    /// `ranges`: An even-length ordered sequence of non-overlapping, non-contiguous, [inclusive
    /// start, exclusive end) ranges.
    // port: CharRanges#withRanges
    pub fn with_ranges(ranges: &[i32]) -> CharRanges {
        if (ranges.len() & 1) != 0 {
            panic!("java.lang.IllegalArgumentException");
        }
        for i in 1..ranges.len() {
            if ranges[i] <= ranges[i - 1] {
                panic!(
                    "java.lang.IllegalArgumentException: {} > {}",
                    ranges[i],
                    ranges[i - 1]
                );
            }
        }
        CharRanges::new(ranges.to_vec())
    }

    // port: CharRanges#CharRanges
    fn new(ranges: Vec<i32>) -> CharRanges {
        CharRanges { ranges }
    }

    // port: CharRanges#intArrayToRanges
    fn int_array_to_ranges(members: &mut [i32]) -> Vec<i32> {
        let n_members = members.len();
        if n_members == 0 {
            return Vec::new();
        }

        members.sort_unstable();

        // Count the number of runs.
        let mut n_runs = 1;
        for i in 1..n_members {
            let current = members[i];
            let last = members[i - 1];
            if current == last {
                continue;
            }
            if current != last.wrapping_add(1) {
                n_runs += 1;
            }
        }

        let mut ranges = vec![0i32; n_runs * 2];
        ranges[0] = members[0];
        let mut k = 0;
        let mut i = 1;
        while k + 2 < ranges.len() {
            let current = members[i];
            let last = members[i - 1];
            if current == last {
                i += 1;
                continue;
            }
            if current != last.wrapping_add(1) {
                k += 1;
                ranges[k] = last.wrapping_add(1); // add 1 to make end exclusive
                k += 1;
                ranges[k] = current;
            }
            i += 1;
        }
        k += 1;
        ranges[k] = members[n_members - 1].wrapping_add(1); // add 1 to make end exclusive
        ranges
    }

    // port: CharRanges#contains
    pub fn contains(&self, bit: i32) -> bool {
        (binary_search(&self.ranges, bit) & 1) == 0
        // By the contract of Arrays.binarySearch, its result is either the position of bit in
        // ranges or it is the bitwise inverse of the position of the least element greater than
        // bit. In all cases, oddness is equivalent to containedness (see the Java source).
    }

    // port: CharRanges#isEmpty
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    // port: CharRanges#getNumRanges
    pub fn get_num_ranges(&self) -> i32 {
        (self.ranges.len() >> 1) as i32
    }

    // port: CharRanges#start
    pub fn start(&self, i: i32) -> i32 {
        self.ranges[(i << 1) as usize]
    }

    // port: CharRanges#end
    pub fn end(&self, i: i32) -> i32 {
        self.ranges[((i << 1) | 1) as usize]
    }

    // port: CharRanges#union
    pub fn union(&self, other: &CharRanges) -> CharRanges {
        // Index of the input ranges
        let q = &self.ranges;
        let r = &other.ranges;
        // Lengths of the inputs
        let m = q.len();
        let n = r.len();

        if m == 0 {
            return other.clone();
        }
        if n == 0 {
            return self.clone();
        }

        // The output array. The length is m+n in the worst case when all the ranges in a are
        // disjoint from the ranges in b.
        let mut out = vec![0i32; m + n];

        // Indexes into the various arrays
        let mut i = 0;
        let mut j = 0;
        let mut k = 0;

        // This loop exits because we always increment at least one of i,j.
        while i < m && j < n {
            // Range starts and ends.
            let a0 = q[i];
            let a1 = q[i + 1];
            let b0 = r[j];
            let b1 = r[j + 1];
            if a1 < b0 {
                // [a0, a1) ends before [b0, b1) starts
                out[k] = a0;
                k += 1;
                out[k] = a1;
                k += 1;
                i += 2;
            } else if b1 < a0 {
                // [b0, b1) ends before [a0, a1) starts
                out[k] = b0;
                k += 1;
                out[k] = b1;
                k += 1;
                j += 2;
            } else {
                // ranges overlap
                // We need to compute a new range based on the set of ranges that transitively
                // overlap.
                let start = a0.min(b0);
                // Guess at the end, and lookahead to come up with a more complete estimate.
                let mut end = a1.max(b1);
                i += 2;
                j += 2;
                while i < m || j < n {
                    if i < m && q[i] <= end {
                        end = end.max(q[i + 1]);
                        i += 2;
                    } else if j < n && r[j] <= end {
                        end = end.max(r[j + 1]);
                        j += 2;
                    } else {
                        break;
                    }
                }
                out[k] = start;
                k += 1;
                out[k] = end;
                k += 1;
            }
        }
        // There may be unprocessed ranges at the end of one of the inputs.
        if i < m {
            out[k..k + (m - i)].copy_from_slice(&q[i..m]);
            k += m - i;
        } else if j < n {
            out[k..k + (n - j)].copy_from_slice(&r[j..n]);
            k += n - j;
        }
        // We guessed at the output length above. Cut off the tail.
        if k != out.len() {
            out.truncate(k);
        }
        CharRanges::new(out)
    }

    // port: CharRanges#intersection
    pub fn intersection(&self, other: &CharRanges) -> CharRanges {
        let a_ranges = &self.ranges;
        let b_ranges = &other.ranges;
        let a_len = a_ranges.len();
        let b_len = b_ranges.len();
        if a_len == 0 {
            return self.clone();
        }
        if b_len == 0 {
            return other.clone();
        }
        let mut a_idx = 0;
        let mut b_idx = 0;
        let mut intersection = vec![0i32; a_len.min(b_len)];
        let mut intersection_idx = 0;
        let mut pos = a_ranges[0].min(b_ranges[0]);
        while a_idx < a_len && b_idx < b_len {
            if a_ranges[a_idx + 1] <= pos {
                a_idx += 2;
            } else if b_ranges[b_idx + 1] <= pos {
                b_idx += 2;
            } else {
                let start = a_ranges[a_idx].max(b_ranges[b_idx]);
                if pos < start {
                    // Advance to start of common block.
                    pos = start;
                } else {
                    // Now we know that pos is less than the ends of the two ranges and greater or
                    // equal to the starts of the two ranges.
                    let end = a_ranges[a_idx + 1].min(b_ranges[b_idx + 1]);
                    if intersection_idx != 0 && pos == intersection[intersection_idx - 1] {
                        intersection[intersection_idx - 1] = end;
                    } else {
                        if intersection_idx == intersection.len() {
                            intersection.resize(intersection_idx * 2, 0);
                        }
                        intersection[intersection_idx] = pos;
                        intersection_idx += 1;
                        intersection[intersection_idx] = end;
                        intersection_idx += 1;
                    }
                    pos = end;
                }
            }
        }
        if intersection_idx != intersection.len() {
            intersection.truncate(intersection_idx);
        }
        CharRanges::new(intersection)
    }

    // port: CharRanges#difference
    pub fn difference(&self, subtrahend_ranges: &CharRanges) -> CharRanges {
        // difference = minuend - subtrahend
        let minuend = &self.ranges;
        let subtrahend = &subtrahend_ranges.ranges;

        let mn = minuend.len();
        let sn = subtrahend.len();
        if mn == 0 || sn == 0 {
            return self.clone();
        }

        let mut difference = vec![0i32; minuend.len()];

        // Indices into minuend.ranges, subtrahend.ranges, and difference.
        let mut m_idx = 0;
        let mut s_idx = 0;
        let mut d_idx = 0;

        let mut pos = minuend[0];
        while m_idx < mn {
            if pos >= minuend[m_idx + 1] {
                m_idx += 2;
            } else if pos < minuend[m_idx] {
                // Skip gaps in the minuend.
                pos = minuend[m_idx];
            } else if s_idx < sn && pos >= subtrahend[s_idx] {
                // Skip over a removed part.
                pos = subtrahend[s_idx + 1];
                s_idx += 2;
            } else {
                // Now we know that pos is between [minuend[i], minuend[i + 1]) and outside
                // [subtrahend[j], subtrahend[j + 1]).
                let end = if s_idx < sn {
                    minuend[m_idx + 1].min(subtrahend[s_idx])
                } else {
                    minuend[m_idx + 1]
                };
                if d_idx != 0 && difference[d_idx - 1] == pos {
                    difference[d_idx - 1] = pos;
                } else {
                    if d_idx == difference.len() {
                        difference.resize(d_idx * 2, 0);
                    }
                    difference[d_idx] = pos;
                    d_idx += 1;
                    difference[d_idx] = end;
                    d_idx += 1;
                }
                pos = end;
            }
        }

        if d_idx != difference.len() {
            difference.truncate(d_idx);
        }

        CharRanges::new(difference)
    }

    // port: CharRanges#containsAll
    pub fn contains_all(&self, sub: &CharRanges) -> bool {
        let super_ranges = &self.ranges;
        let sub_ranges = &sub.ranges;

        let mut super_idx = 0;
        let mut sub_idx = 0;
        let super_len = super_ranges.len();
        let sub_len = sub_ranges.len();
        while sub_idx < sub_len {
            if super_idx == super_len {
                return false;
            }
            if super_ranges[super_idx + 1] <= sub_ranges[sub_idx] {
                // Super range ends before subRange starts.
                super_idx += 2;
            } else if super_ranges[super_idx] > sub_ranges[sub_idx] {
                // Uncontained portion at start of sub range.
                return false;
            } else if super_ranges[super_idx + 1] >= sub_ranges[sub_idx + 1] {
                // A sub range is completely contained in the super range.
                sub_idx += 2;
            } else {
                // Uncontained portion at end of sub range.
                return false;
            }
        }
        sub_idx == sub_len
    }

    /// Shifts the bits matched by the given delta. So if this has the bits (a, b, c, ..., z) set
    /// then the result has the bits ((a - delta), (b - delta), (c - delta), ...., (z - delta)) set.
    ///
    /// Java documents an IndexOutOfBoundsException on int overflow, but its check adds in `int`
    /// before widening to `long`, so it never fires; the wrapping is kept.
    // port: CharRanges#shift
    pub fn shift(&self, delta: i32) -> CharRanges {
        let n = self.ranges.len();
        if delta == 0 || n == 0 {
            return self.clone();
        }
        // Test overflow/underflow
        if delta < 0 {
            let lmin = self.ranges[0].wrapping_add(delta) as i64;
            if lmin < i32::MIN as i64 {
                panic!("java.lang.IndexOutOfBoundsException");
            }
        } else {
            let lmax = self.ranges[n - 1].wrapping_add(delta) as i64;
            if lmax > i32::MAX as i64 {
                panic!("java.lang.IndexOutOfBoundsException");
            }
        }
        // Create a shifted range.
        let mut shifted_ranges = vec![0i32; n];
        let mut i = n;
        while i > 0 {
            i -= 1;
            shifted_ranges[i] = self.ranges[i].wrapping_add(delta);
        }
        CharRanges::new(shifted_ranges)
    }

    // port: CharRanges#hashCode
    pub fn hash_code(&self) -> i32 {
        let mut hc: i32 = 0;
        let n = 16.min(self.ranges.len());
        for i in 0..n {
            hc = (hc << 2).wrapping_add(self.ranges[i]);
        }
        hc
    }
}

// port: CharRanges#toString
impl fmt::Display for CharRanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sb = String::new();
        sb.push('[');
        for i in 0..self.ranges.len() {
            if (i & 1) != 0 && self.ranges[i] == self.ranges[i - 1].wrapping_add(1) {
                continue;
            }
            if i != 0 {
                sb.push(if (i & 1) == 0 { ' ' } else { '-' });
            }
            sb.push_str("0x");
            sb.push_str(&java_lang::to_string_radix(
                self.ranges[i].wrapping_sub((i & 1) as i32),
                16,
            ));
        }
        sb.push(']');
        f.write_str(&sb)
    }
}

// port: Arrays#binarySearch(int[], int)
fn binary_search(a: &[i32], key: i32) -> i32 {
    let mut low: i32 = 0;
    let mut high: i32 = a.len() as i32 - 1;

    while low <= high {
        let mid = ((low + high) as u32 >> 1) as i32;
        let mid_val = a[mid as usize];

        if mid_val < key {
            low = mid + 1;
        } else if mid_val > key {
            high = mid - 1;
        } else {
            return mid; // key found
        }
    }
    -(low + 1) // key not found.
}
