// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::RangeSummary;
use crate::{Entry, Summary};
use alloc::vec::Vec;
use core::mem::size_of;

const GORILLA_ANCHOR_STRIDE: usize = 64;

/// Encoding used for sealed chronology chunks.
///
/// A codec owns the byte shape and decode mechanics for closed chunks. The
/// chronology still owns append order, retention, chunk summaries, and range
/// semantics.
pub trait ChunkCodec<V: Copy> {
    /// Number of entries covered by one sealed-chunk summary tile.
    ///
    /// Codecs with periodic decoder anchors should generally use the same
    /// stride here so summary tiles and exact-decode restart points align.
    const SUMMARY_TILE_CAPACITY: usize;

    /// Encoded representation stored inside each sealed chunk.
    type Encoded;

    /// Encode one sealed chunk.
    ///
    /// The provided columns have the same length and are sorted by timestamp.
    fn encode(timestamps: Vec<u64>, values: Vec<V>) -> Self::Encoded;

    /// Return the encoded payload size, in bytes.
    ///
    /// This reports the logical payload size used for storage comparison. It
    /// does not include allocator capacity or the outer chunk metadata.
    fn encoded_size(encoded: &Self::Encoded) -> usize;

    /// Return the first entry at or after `timestamp`.
    fn entry_at_or_after(encoded: &Self::Encoded, timestamp: u64) -> Option<Entry<V>>;

    /// Return the last entry at or before `timestamp`.
    fn entry_at_or_before(encoded: &Self::Encoded, timestamp: u64) -> Option<Entry<V>>;

    /// Add the entries in `start..end` to `range_summary`.
    fn add_range_summary<S: Summary<V>>(
        encoded: &Self::Encoded,
        start: u64,
        end: u64,
        range_summary: &mut RangeSummary<S>,
    );
}

/// Codec that stores sealed chunks as raw timestamp and value columns.
#[derive(Clone, Copy, Debug, Default)]
pub struct RawCodec;

/// Raw encoded sealed chunk payload.
pub struct RawEncodedChunk<V: Copy> {
    timestamps: Vec<u64>,
    values: Vec<V>,
}

impl<V: Copy> ChunkCodec<V> for RawCodec {
    const SUMMARY_TILE_CAPACITY: usize = 64;

    type Encoded = RawEncodedChunk<V>;

    fn encode(timestamps: Vec<u64>, values: Vec<V>) -> Self::Encoded {
        RawEncodedChunk { timestamps, values }
    }

    fn encoded_size(encoded: &Self::Encoded) -> usize {
        encoded.timestamps.len() * size_of::<u64>() + encoded.values.len() * size_of::<V>()
    }

    fn entry_at_or_after(encoded: &Self::Encoded, timestamp: u64) -> Option<Entry<V>> {
        let index = first_index_at_least(&encoded.timestamps, timestamp)?;
        Some(entry(encoded, index))
    }

    fn entry_at_or_before(encoded: &Self::Encoded, timestamp: u64) -> Option<Entry<V>> {
        let index = last_index_at_most(&encoded.timestamps, timestamp)?;
        Some(entry(encoded, index))
    }

    fn add_range_summary<S: Summary<V>>(
        encoded: &Self::Encoded,
        start: u64,
        end: u64,
        range_summary: &mut RangeSummary<S>,
    ) {
        let start_index = lower_bound(&encoded.timestamps, start);
        let end_index = lower_bound(&encoded.timestamps, end);
        if start_index == end_index {
            return;
        }

        let mut summary = S::default();
        for index in start_index..end_index {
            summary.update(&entry(encoded, index));
        }
        range_summary.add_summary::<V>(end_index - start_index, &summary);
    }
}

/// Gorilla-inspired codec for `f64` sealed chunks.
///
/// Timestamps are stored as varint-encoded delta-of-delta values. Values store
/// the first IEEE-754 bit pattern and then use Gorilla-style XOR control bits
/// for later values. The codec also stores periodic decoder anchors so random
/// access only has to replay from the nearest anchor within a sealed chunk.
#[derive(Clone, Copy, Debug, Default)]
pub struct GorillaF64Codec;

/// Encoded sealed chunk payload produced by [`GorillaF64Codec`].
pub struct GorillaF64EncodedChunk {
    len: usize,
    first_timestamp: u64,
    first_delta: u64,
    timestamp_deltas: Vec<u8>,
    timestamp_anchors: Vec<TimestampAnchor>,
    first_value_bits: u64,
    value_bits: Vec<u8>,
    value_anchors: Vec<ValueAnchor>,
}

impl ChunkCodec<f64> for GorillaF64Codec {
    const SUMMARY_TILE_CAPACITY: usize = GORILLA_ANCHOR_STRIDE;

    type Encoded = GorillaF64EncodedChunk;

    fn encode(timestamps: Vec<u64>, values: Vec<f64>) -> Self::Encoded {
        debug_assert_eq!(timestamps.len(), values.len());
        debug_assert!(!timestamps.is_empty());

        let first_timestamp = timestamps[0];
        let (first_delta, timestamp_deltas, timestamp_anchors) =
            encode_timestamp_deltas(&timestamps);
        let first_value_bits = values[0].to_bits();
        let (value_bits, value_anchors) = encode_value_bits(&values);

        GorillaF64EncodedChunk {
            len: timestamps.len(),
            first_timestamp,
            first_delta,
            timestamp_deltas,
            timestamp_anchors,
            first_value_bits,
            value_bits,
            value_anchors,
        }
    }

    fn encoded_size(encoded: &Self::Encoded) -> usize {
        size_of::<usize>()
            + size_of::<u64>() * 3
            + encoded.timestamp_deltas.len()
            + encoded.timestamp_anchors.len() * size_of::<TimestampAnchor>()
            + encoded.value_bits.len()
            + encoded.value_anchors.len() * size_of::<ValueAnchor>()
    }

    fn entry_at_or_after(encoded: &Self::Encoded, timestamp: u64) -> Option<Entry<f64>> {
        entry_iter_at_or_before(encoded, timestamp)
            .find(|entry| entry.timestamp >= timestamp)
            .map(|entry| Entry::new(entry.timestamp, entry.value))
    }

    fn entry_at_or_before(encoded: &Self::Encoded, timestamp: u64) -> Option<Entry<f64>> {
        let mut previous = None;
        for entry in entry_iter_at_or_before(encoded, timestamp) {
            if entry.timestamp > timestamp {
                break;
            }
            previous = Some(Entry::new(entry.timestamp, entry.value));
        }
        previous
    }

    fn add_range_summary<S: Summary<f64>>(
        encoded: &Self::Encoded,
        start: u64,
        end: u64,
        range_summary: &mut RangeSummary<S>,
    ) {
        let mut summary = S::default();
        let mut len = 0;

        for entry in entry_iter_at_or_before(encoded, start) {
            if entry.timestamp >= end {
                break;
            }
            if entry.timestamp >= start {
                summary.update(&entry);
                len += 1;
            }
        }

        range_summary.add_summary::<f64>(len, &summary);
    }
}

fn entry<V: Copy>(encoded: &RawEncodedChunk<V>, index: usize) -> Entry<V> {
    Entry::new(encoded.timestamps[index], encoded.values[index])
}

fn first_index_at_least(timestamps: &[u64], timestamp: u64) -> Option<usize> {
    let index = lower_bound(timestamps, timestamp);
    (index < timestamps.len()).then_some(index)
}

fn last_index_at_most(timestamps: &[u64], timestamp: u64) -> Option<usize> {
    match timestamps.binary_search(&timestamp) {
        Ok(index) => Some(index),
        Err(index) => index.checked_sub(1),
    }
}

fn lower_bound(timestamps: &[u64], timestamp: u64) -> usize {
    match timestamps.binary_search(&timestamp) {
        Ok(index) | Err(index) => index,
    }
}

fn encode_timestamp_deltas(timestamps: &[u64]) -> (u64, Vec<u8>, Vec<TimestampAnchor>) {
    let first_delta = timestamps
        .get(1)
        .map(|timestamp| timestamp - timestamps[0])
        .unwrap_or(0);
    let mut previous_delta = 0;
    let mut timestamp_deltas = Vec::new();
    let mut anchors = Vec::new();

    for index in 1..timestamps.len() {
        let delta = timestamps[index] - timestamps[index - 1];
        if index > 1 {
            let delta_of_delta = i128::from(delta) - i128::from(previous_delta);
            encode_var_u128(&mut timestamp_deltas, zigzag_encode(delta_of_delta));
        }
        previous_delta = delta;

        if index.is_multiple_of(GORILLA_ANCHOR_STRIDE) {
            anchors.push(TimestampAnchor {
                index,
                timestamp: timestamps[index],
                delta,
                offset: timestamp_deltas.len(),
            });
        }
    }

    (first_delta, timestamp_deltas, anchors)
}

fn encode_value_bits(values: &[f64]) -> (Vec<u8>, Vec<ValueAnchor>) {
    let mut writer = BitWriter::new();
    let mut anchors = Vec::new();
    let mut previous_bits = values[0].to_bits();
    let mut previous_leading = 0;
    let mut previous_trailing = 0;
    let mut has_previous_window = false;

    for (offset, value) in values[1..].iter().enumerate() {
        let index = offset + 1;
        let value_bits = value.to_bits();
        let xor = previous_bits ^ value_bits;
        if xor == 0 {
            writer.write_bit(false);
        } else {
            writer.write_bit(true);
            let leading = xor.leading_zeros().min(31);
            let trailing = xor.trailing_zeros();
            if has_previous_window && leading >= previous_leading && trailing >= previous_trailing {
                writer.write_bit(false);
                let width = 64 - previous_leading - previous_trailing;
                writer.write_bits(significant_bits(xor, previous_trailing, width), width);
            } else {
                writer.write_bit(true);
                let width = 64 - leading - trailing;
                writer.write_bits(u64::from(leading), 5);
                writer.write_bits(if width == 64 { 0 } else { u64::from(width) }, 6);
                writer.write_bits(significant_bits(xor, trailing, width), width);

                previous_leading = leading;
                previous_trailing = trailing;
                has_previous_window = true;
            }
        }
        previous_bits = value_bits;

        if index.is_multiple_of(GORILLA_ANCHOR_STRIDE) {
            anchors.push(ValueAnchor {
                index,
                value_bits,
                leading: previous_leading,
                trailing: previous_trailing,
                bit_index: writer.bit_len(),
                has_window: has_previous_window,
            });
        }
    }

    (writer.into_bytes(), anchors)
}

fn significant_bits(value: u64, trailing: u32, width: u32) -> u64 {
    if width == 64 {
        value
    } else {
        (value >> trailing) & ((1_u64 << width) - 1)
    }
}

fn entry_iter_at_or_before(encoded: &GorillaF64EncodedChunk, timestamp: u64) -> EntryIter<'_> {
    EntryIter::from_anchor_slot(encoded, anchor_slot_at_or_before(encoded, timestamp))
}

struct EntryIter<'a> {
    timestamps: TimestampIter<'a>,
    values: ValueIter<'a>,
}

impl<'a> EntryIter<'a> {
    fn from_anchor_slot(encoded: &'a GorillaF64EncodedChunk, slot: Option<usize>) -> Self {
        debug_assert_eq!(encoded.timestamp_anchors.len(), encoded.value_anchors.len());

        EntryIter {
            timestamps: TimestampIter::from_anchor(encoded, timestamp_anchor(encoded, slot)),
            values: ValueIter::from_anchor(encoded, value_anchor(encoded, slot)),
        }
    }
}

impl Iterator for EntryIter<'_> {
    type Item = Entry<f64>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(Entry::new(self.timestamps.next()?, self.values.next()?))
    }
}

#[derive(Clone, Copy)]
struct TimestampAnchor {
    index: usize,
    timestamp: u64,
    delta: u64,
    offset: usize,
}

struct TimestampIter<'a> {
    encoded: &'a GorillaF64EncodedChunk,
    index: usize,
    previous_timestamp: u64,
    previous_delta: u64,
    offset: usize,
    pending_anchor: bool,
}

impl<'a> TimestampIter<'a> {
    fn from_anchor(encoded: &'a GorillaF64EncodedChunk, anchor: TimestampAnchor) -> Self {
        TimestampIter {
            encoded,
            index: anchor.index,
            previous_timestamp: anchor.timestamp,
            previous_delta: anchor.delta,
            offset: anchor.offset,
            pending_anchor: true,
        }
    }
}

impl Iterator for TimestampIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.encoded.len {
            return None;
        }

        if self.pending_anchor {
            self.pending_anchor = false;
            self.index += 1;
            return Some(self.previous_timestamp);
        }

        let timestamp = match self.index {
            0 => self.encoded.first_timestamp,
            1 => {
                self.previous_delta = self.encoded.first_delta;
                self.previous_timestamp = self
                    .encoded
                    .first_timestamp
                    .checked_add(self.encoded.first_delta)?;
                self.previous_timestamp
            }
            _ => {
                let encoded_delta =
                    decode_var_u128(&self.encoded.timestamp_deltas, &mut self.offset)?;
                let delta_of_delta = zigzag_decode(encoded_delta);
                let delta = (i128::from(self.previous_delta) + delta_of_delta) as u64;
                self.previous_delta = delta;
                self.previous_timestamp = self.previous_timestamp.checked_add(delta)?;
                self.previous_timestamp
            }
        };

        self.index += 1;
        Some(timestamp)
    }
}

#[derive(Clone, Copy)]
struct ValueAnchor {
    index: usize,
    value_bits: u64,
    leading: u32,
    trailing: u32,
    bit_index: usize,
    has_window: bool,
}

struct ValueIter<'a> {
    encoded: &'a GorillaF64EncodedChunk,
    index: usize,
    reader: BitReader<'a>,
    previous_bits: u64,
    leading: u32,
    trailing: u32,
    has_window: bool,
    pending_anchor: bool,
}

impl<'a> ValueIter<'a> {
    fn from_anchor(encoded: &'a GorillaF64EncodedChunk, anchor: ValueAnchor) -> Self {
        ValueIter {
            encoded,
            index: anchor.index,
            reader: BitReader::with_bit_index(&encoded.value_bits, anchor.bit_index),
            previous_bits: anchor.value_bits,
            leading: anchor.leading,
            trailing: anchor.trailing,
            has_window: anchor.has_window,
            pending_anchor: true,
        }
    }
}

impl Iterator for ValueIter<'_> {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.encoded.len {
            return None;
        }

        if self.pending_anchor {
            self.pending_anchor = false;
            self.index += 1;
            return Some(f64::from_bits(self.previous_bits));
        }

        if self.index == 0 {
            self.index += 1;
            return Some(f64::from_bits(self.previous_bits));
        }

        if self.reader.read_bit()? {
            if self.reader.read_bit()? || !self.has_window {
                self.leading = self.reader.read_bits(5)? as u32;
                let width = match self.reader.read_bits(6)? as u32 {
                    0 => 64,
                    width => width,
                };
                self.trailing = 64 - self.leading - width;
                let payload = self.reader.read_bits(width)?;
                self.previous_bits ^= payload << self.trailing;
                self.has_window = true;
            } else {
                let width = 64 - self.leading - self.trailing;
                let payload = self.reader.read_bits(width)?;
                self.previous_bits ^= payload << self.trailing;
            }
        }

        self.index += 1;
        Some(f64::from_bits(self.previous_bits))
    }
}

struct BitWriter {
    bytes: Vec<u8>,
    current: u8,
    filled: u8,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter {
            bytes: Vec::new(),
            current: 0,
            filled: 0,
        }
    }

    fn write_bit(&mut self, value: bool) {
        self.current = (self.current << 1) | u8::from(value);
        self.filled += 1;

        if self.filled == 8 {
            self.bytes.push(self.current);
            self.current = 0;
            self.filled = 0;
        }
    }

    fn write_bits(&mut self, value: u64, count: u32) {
        for shift in (0..count).rev() {
            self.write_bit(((value >> shift) & 1) != 0);
        }
    }

    fn bit_len(&self) -> usize {
        self.bytes.len() * 8 + usize::from(self.filled)
    }

    fn into_bytes(mut self) -> Vec<u8> {
        if self.filled > 0 {
            self.current <<= 8 - self.filled;
            self.bytes.push(self.current);
        }
        self.bytes
    }
}

struct BitReader<'a> {
    bytes: &'a [u8],
    bit_index: usize,
}

impl<'a> BitReader<'a> {
    fn with_bit_index(bytes: &'a [u8], bit_index: usize) -> Self {
        BitReader { bytes, bit_index }
    }

    fn read_bit(&mut self) -> Option<bool> {
        let byte = *self.bytes.get(self.bit_index / 8)?;
        let shift = 7 - (self.bit_index % 8);
        self.bit_index += 1;
        Some(((byte >> shift) & 1) != 0)
    }

    fn read_bits(&mut self, count: u32) -> Option<u64> {
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1) | u64::from(self.read_bit()?);
        }
        Some(value)
    }
}

fn encode_var_u128(output: &mut Vec<u8>, mut value: u128) {
    while value >= 0x80 {
        output.push(((value & 0x7f) as u8) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

fn decode_var_u128(input: &[u8], offset: &mut usize) -> Option<u128> {
    let mut value = 0;
    let mut shift = 0;

    loop {
        let byte = *input.get(*offset)?;
        *offset += 1;
        value |= u128::from(byte & 0x7f) << shift;

        if byte & 0x80 == 0 {
            return Some(value);
        }

        shift += 7;
        if shift >= 128 {
            return None;
        }
    }
}

fn zigzag_encode(value: i128) -> u128 {
    ((value << 1) ^ (value >> 127)) as u128
}

fn zigzag_decode(value: u128) -> i128 {
    ((value >> 1) as i128) ^ -((value & 1) as i128)
}

fn anchor_slot_at_or_before(encoded: &GorillaF64EncodedChunk, timestamp: u64) -> Option<usize> {
    match encoded
        .timestamp_anchors
        .binary_search_by_key(&timestamp, |anchor| anchor.timestamp)
    {
        Ok(index) => Some(index),
        Err(0) => None,
        Err(index) => Some(index - 1),
    }
}

fn timestamp_anchor(encoded: &GorillaF64EncodedChunk, slot: Option<usize>) -> TimestampAnchor {
    slot.map(|index| encoded.timestamp_anchors[index])
        .unwrap_or(TimestampAnchor {
            index: 0,
            timestamp: encoded.first_timestamp,
            delta: 0,
            offset: 0,
        })
}

fn value_anchor(encoded: &GorillaF64EncodedChunk, slot: Option<usize>) -> ValueAnchor {
    slot.map(|index| encoded.value_anchors[index])
        .unwrap_or(ValueAnchor {
            index: 0,
            value_bits: encoded.first_value_bits,
            leading: 0,
            trailing: 0,
            bit_index: 0,
            has_window: false,
        })
}
