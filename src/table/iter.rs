use super::{KeyMissing, Table};
use crate::Value;
use crate::value::UnsafeValue;

/// Implementation of [Iterator] to enumerate all key-values from a table.
pub struct Iter<'a, A> {
    tab: &'a Table<A>,
    key: Value<'a, A>, // Cannot be UnsafeValue since it may removed from the table.
}

impl<'a, A> Iter<'a, A> {
    #[inline(always)]
    pub(super) fn new(tab: &'a Table<A>) -> Self {
        Self {
            tab,
            key: Value::Nil,
        }
    }
}

impl<'a, A> Iterator for Iter<'a, A> {
    type Item = Result<(Value<'a, A>, Value<'a, A>), KeyMissing>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let k = UnsafeValue::from(&self.key);
        let [k, v] = match unsafe { self.tab.next_raw(&k) } {
            Ok(Some(v)) => v,
            Ok(None) => return None,
            Err(e) => return Some(Err(e)),
        };

        self.key = unsafe { Value::from_unsafe(&k) };

        Some(Ok((self.key.clone(), unsafe { Value::from_unsafe(&v) })))
    }
}
