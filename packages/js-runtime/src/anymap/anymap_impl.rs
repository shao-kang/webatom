use std::any::{Any, TypeId};
use std::collections::{hash_map, HashMap};
use std::fmt;
use std::marker::PhantomData;

// ── UnsafeAny ─────────────────────────────────────────────────────────────────

/// # Safety
/// Only implement for `dyn Any`, `dyn Any + Send`, `dyn Any + Send + Sync`.
/// The fat-pointer data component always equals `*const T` for the concrete type T.
pub unsafe trait UnsafeAny: Any {}

// SAFETY: for all three variants the fat-pointer layout is (data_ptr, vtable_ptr),
// and data_ptr == address of the concrete T value, so pointer-casting is valid.
unsafe impl UnsafeAny for dyn Any {}
unsafe impl UnsafeAny for dyn Any + Send {}
unsafe impl UnsafeAny for dyn Any + Send + Sync {}

// ── IntoBox ───────────────────────────────────────────────────────────────────

/// Coerce a concrete value into the boxed trait object stored in `Map<A>`.
///
/// Implemented for the three standard variants automatically — callers never
/// need to name this trait; it's used as a bound in `Map<A>` methods.
pub trait IntoBox<A: ?Sized + UnsafeAny>: Any {
    fn into_box(self) -> Box<A>;
}

impl<T: Any> IntoBox<dyn Any> for T {
    fn into_box(self) -> Box<dyn Any> {
        Box::new(self)
    }
}

impl<T: Any + Send> IntoBox<dyn Any + Send> for T {
    fn into_box(self) -> Box<dyn Any + Send> {
        Box::new(self)
    }
}

impl<T: Any + Send + Sync> IntoBox<dyn Any + Send + Sync> for T {
    fn into_box(self) -> Box<dyn Any + Send + Sync> {
        Box::new(self)
    }
}

// ── Map ───────────────────────────────────────────────────────────────────────

/// A map keyed by `TypeId` where each type `T` maps to exactly one value of type `T`.
///
/// The type parameter `A` controls the send/sync bounds on stored values:
///
/// | Alias | Bound on values |
/// |---|---|
/// | [`AnyMap`] | none (`dyn Any`) |
/// | [`SendAnyMap`] | `Send` |
/// | [`SyncAnyMap`] | `Send + Sync` |
pub struct Map<A: ?Sized + UnsafeAny = dyn Any> {
    raw: HashMap<TypeId, Box<A>>,
}

/// [`Map`] over `dyn Any` — no thread-safety requirements on values.
pub type AnyMap = Map<dyn Any>;

/// [`Map`] over `dyn Any + Send`.
pub type SendAnyMap = Map<dyn Any + Send>;

/// [`Map`] over `dyn Any + Send + Sync` — values can be shared across threads.
pub type SyncAnyMap = Map<dyn Any + Send + Sync>;

impl<A: ?Sized + UnsafeAny> Map<A> {
    pub fn new() -> Self {
        Map {
            raw: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Map {
            raw: HashMap::with_capacity(capacity),
        }
    }

    /// Returns a reference to the value of type `T`, or `None`.
    pub fn get<T: IntoBox<A>>(&self) -> Option<&T> {
        self.raw.get(&TypeId::of::<T>()).map(|b| {
            // SAFETY: key is TypeId::of::<T>(), so concrete type behind the fat ptr is T.
            // Casting *const dyn Trait → *const T extracts the data pointer.
            unsafe { &*(b.as_ref() as *const A as *const T) }
        })
    }

    /// Returns a mutable reference to the value of type `T`, or `None`.
    pub fn get_mut<T: IntoBox<A>>(&mut self) -> Option<&mut T> {
        self.raw.get_mut(&TypeId::of::<T>()).map(|b| {
            // SAFETY: same as get
            unsafe { &mut *(b.as_mut() as *mut A as *mut T) }
        })
    }

    /// Inserts `value`, returning the previous value of type `T` if one existed.
    pub fn insert<T: IntoBox<A>>(&mut self, value: T) -> Option<T> {
        self.raw
            .insert(TypeId::of::<T>(), value.into_box())
            .map(|old| {
                // SAFETY: key was TypeId::of::<T>()
                unsafe { *Box::from_raw(Box::into_raw(old) as *mut T) }
            })
    }

    /// Removes and returns the value of type `T`, or `None`.
    pub fn remove<T: IntoBox<A>>(&mut self) -> Option<T> {
        self.raw.remove(&TypeId::of::<T>()).map(|b| {
            // SAFETY: key was TypeId::of::<T>()
            unsafe { *Box::from_raw(Box::into_raw(b) as *mut T) }
        })
    }

    pub fn contains<T: IntoBox<A>>(&self) -> bool {
        self.raw.contains_key(&TypeId::of::<T>())
    }

    /// Entry API for insert-or-init patterns, e.g. `.entry::<T>().or_default()`.
    pub fn entry<T: IntoBox<A>>(&mut self) -> Entry<'_, A, T> {
        Entry {
            inner: self.raw.entry(TypeId::of::<T>()),
            _marker: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.raw.len()
    }

    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    pub fn clear(&mut self) {
        self.raw.clear();
    }
}

impl<A: ?Sized + UnsafeAny> Default for Map<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: ?Sized + UnsafeAny> fmt::Debug for Map<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Map")
            .field("len", &self.raw.len())
            .finish_non_exhaustive()
    }
}

// ── Entry ─────────────────────────────────────────────────────────────────────

pub struct Entry<'a, A: ?Sized + UnsafeAny, T: IntoBox<A>> {
    inner: hash_map::Entry<'a, TypeId, Box<A>>,
    _marker: PhantomData<fn() -> T>,
}

impl<'a, A: ?Sized + UnsafeAny, T: IntoBox<A>> Entry<'a, A, T> {
    pub fn or_insert(self, default: T) -> &'a mut T {
        self.or_insert_with(|| default)
    }

    pub fn or_insert_with(self, f: impl FnOnce() -> T) -> &'a mut T {
        let b = self.inner.or_insert_with(|| f().into_box());
        // SAFETY: key is TypeId::of::<T>()
        unsafe { &mut *(b.as_mut() as *mut A as *mut T) }
    }

    pub fn or_default(self) -> &'a mut T
    where
        T: Default,
    {
        self.or_insert_with(T::default)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut map = AnyMap::new();
        map.insert(42u32);
        assert_eq!(map.get::<u32>(), Some(&42));
        assert_eq!(map.get::<u64>(), None);
    }

    #[test]
    fn insert_returns_previous() {
        let mut map = AnyMap::new();
        assert_eq!(map.insert(1u32), None);
        assert_eq!(map.insert(2u32), Some(1u32));
        assert_eq!(map.get::<u32>(), Some(&2));
    }

    #[test]
    fn remove() {
        let mut map = AnyMap::new();
        map.insert("hello".to_string());
        assert_eq!(map.remove::<String>(), Some("hello".to_string()));
        assert_eq!(map.remove::<String>(), None);
    }

    #[test]
    fn contains() {
        let mut map = AnyMap::new();
        map.insert(true);
        assert!(map.contains::<bool>());
        assert!(!map.contains::<u8>());
    }

    #[test]
    fn get_mut() {
        let mut map = AnyMap::new();
        map.insert(10u32);
        *map.get_mut::<u32>().unwrap() += 5;
        assert_eq!(map.get::<u32>(), Some(&15));
    }

    #[test]
    fn entry_or_default_accumulates() {
        let mut map = AnyMap::new();
        *map.entry::<u32>().or_default() += 10;
        *map.entry::<u32>().or_default() += 5;
        assert_eq!(map.get::<u32>(), Some(&15));
    }

    #[test]
    fn multiple_types_independent() {
        let mut map = AnyMap::new();
        map.insert(1u8);
        map.insert(2u16);
        map.insert(3u32);
        assert_eq!(map.get::<u8>(), Some(&1));
        assert_eq!(map.get::<u16>(), Some(&2));
        assert_eq!(map.get::<u32>(), Some(&3));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn sync_map() {
        let mut map = SyncAnyMap::new();
        map.insert(42i32);
        assert_eq!(map.get::<i32>(), Some(&42));
    }

    #[test]
    fn clear() {
        let mut map = AnyMap::new();
        map.insert(1u32);
        map.insert("x".to_string());
        map.clear();
        assert!(map.is_empty());
    }
}
