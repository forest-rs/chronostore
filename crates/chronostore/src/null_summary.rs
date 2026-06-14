// Copyright 2026 the Chronostore Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Entry, Summary};
use core::marker::PhantomData;

/// A [`Summary`] that does nothing.
///
/// Use `NullSummary<V>` as the summary parameter for a
/// [`Chronology`](crate::Chronology) when exact samples are needed but
/// aggregate range summaries are not.
#[derive(Debug)]
pub struct NullSummary<V> {
    phantom: PhantomData<V>,
}

impl<V> Clone for NullSummary<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for NullSummary<V> {}

impl<V> Default for NullSummary<V> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<V> Summary<V> for NullSummary<V> {
    fn update(&mut self, _entry: &Entry<V>) {}

    fn merge(&mut self, _other: &Self) {}
}
