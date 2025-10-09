use crate::Lua;
use core::hash::Hasher;

/// Implementation of [`Hasher`] using Lua hashing algorithm.
pub struct LuaHasher(u32);

impl LuaHasher {
    #[inline(always)]
    pub unsafe fn new<D>(g: *const Lua<D>) -> Self {
        Self(unsafe { (*g).seed })
    }
}

impl Hasher for LuaHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.0.into()
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= (self.0 << 5)
                .wrapping_add(self.0 >> 2)
                .wrapping_add(b.into());
        }
    }

    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.0 = self.0.wrapping_add(i as u32);
        self.0 = self.0.wrapping_add((i >> 32) as u32);
    }
}
