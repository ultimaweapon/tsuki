use super::{Collection, CollectionValue, Header};
use crate::Lua;
use crate::value::UnsafeValue;
use core::alloc::Layout;
use core::borrow::Borrow;
use core::cell::RefCell;
use core::marker::PhantomData;
use core::mem::transmute;
use core::ptr::addr_of_mut;

/// Rust [BTreeMap](alloc::collections::btree_map::BTreeMap) to map value of `K` to Lua value `V`.
#[repr(C)]
pub struct BTreeMap<A, K, V> {
    hdr: Header<A>,
    items: RefCell<alloc::collections::btree_map::BTreeMap<K, UnsafeValue<A>>>,
    phantom: PhantomData<V>,
}

impl<A, K, V> BTreeMap<A, K, V>
where
    V: CollectionValue<A>,
{
    pub(crate) unsafe fn new(g: *const Lua<A>) -> *const Self {
        // Create object.
        let layout = Layout::new::<Self>();
        let o = unsafe { (*g).gc.alloc(14 | 1 << 4, layout).cast::<Self>() };

        unsafe { addr_of_mut!((*o).hdr.ptr).write(o as *const dyn Collection) };
        unsafe { addr_of_mut!((*o).items).write(RefCell::default()) };

        o
    }

    /// Returns `true` if the map contains no values.
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// Returns the number of values in the map.
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// Remove all values.
    pub fn clear(&self) {
        self.items.borrow_mut().clear();
    }

    /// Returns `true` if the map contains `k`.
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        K: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.items.borrow().contains_key(k)
    }

    /// Returns a value of `k`.
    pub fn get<Q>(&self, k: &Q) -> Option<V::Out<'_>>
    where
        K: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.items
            .borrow()
            .get(k)
            .map(|v| unsafe { V::from_collection(v) })
    }

    /// Inserts a value into the map.
    ///
    /// Returns previous value for `k`.
    pub fn insert<'a>(&self, k: K, v: impl Into<V::In<'a>>) -> Option<V::Out<'_>>
    where
        K: Ord,
        A: 'a,
    {
        let v = v.into();
        let v = Into::<UnsafeValue<A>>::into(v);
        let p = self.items.borrow_mut().insert(k, v);

        if (v.tt_ & 1 << 6 != 0) && (self.hdr.obj.marked.get() & 1 << 5 != 0) {
            if unsafe { (*v.value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0 } {
                unsafe { self.hdr.obj.global().gc.barrier_back(transmute(self)) };
            }
        }

        p.map(|v| unsafe { V::from_collection(&v) })
    }

    /// Removes a value from the map.
    ///
    /// Return [None] if `k` does not exists.
    pub fn remove<Q>(&self, k: &Q) -> Option<V::Out<'_>>
    where
        K: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.items
            .borrow_mut()
            .remove(k)
            .map(|v| unsafe { V::from_collection(&v) })
    }
}

impl<A, K, V> Collection for BTreeMap<A, K, V> {
    fn mark_items(&self) {
        // This method should never recursive itself to we use borrow_mut to detect that.
        let g = self.hdr.obj.global();

        for v in self.items.borrow_mut().values() {
            let o = unsafe { v.value_.gc };

            if unsafe { v.tt_ & 1 << 6 != 0 && (*o).marked.is_white() } {
                unsafe { g.gc.mark(o) };
            }
        }
    }
}
