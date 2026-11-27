// SPDX-License-Identifier: MIT
use core::marker::PhantomData;

/// A generational handle into an [`Arena`]. A stale handle (its slot was freed and reused) never
/// resolves to the new occupant.
pub struct Handle<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, o: &Self) -> bool {
        self.index == o.index && self.generation == o.generation
    }
}
impl<T> Eq for Handle<T> {}
impl<T> core::hash::Hash for Handle<T> {
    fn hash<H: core::hash::Hasher>(&self, h: &mut H) {
        self.index.hash(h);
        self.generation.hash(h);
    }
}
impl<T> core::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Handle({}v{})", self.index, self.generation)
    }
}

enum Slot<T> {
    Occupied {
        generation: u32,
        value: T,
    },
    Free {
        generation: u32,
        next_free: Option<u32>,
    },
}

/// A slot arena with generational handles. Insertion and removal are O(1).
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free_head: Option<u32>,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    /// An empty arena.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            len: 0,
        }
    }

    /// Number of live values.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when no value is live.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Stores `value` and returns its handle, or `None` when the arena is full (2^32 slots).
    pub fn insert(&mut self, value: T) -> Option<Handle<T>> {
        if let Some(index) = self.free_head {
            let slot = self.slots.get_mut(index as usize)?;
            let Slot::Free {
                generation,
                next_free,
            } = slot
            else {
                return None;
            };
            let generation = *generation;
            let next = *next_free;
            *slot = Slot::Occupied { generation, value };
            self.free_head = next;
            self.len += 1;
            return Some(Handle {
                index,
                generation,
                _marker: PhantomData,
            });
        }
        let index = u32::try_from(self.slots.len()).ok()?;
        self.slots.push(Slot::Occupied {
            generation: 0,
            value,
        });
        self.len += 1;
        Some(Handle {
            index,
            generation: 0,
            _marker: PhantomData,
        })
    }

    /// The value behind `h`, if it is still live.
    pub fn get(&self, h: Handle<T>) -> Option<&T> {
        match self.slots.get(h.index as usize)? {
            Slot::Occupied { generation, value } if *generation == h.generation => Some(value),
            _ => None,
        }
    }

    /// Mutable access to the value behind `h`, if it is still live.
    pub fn get_mut(&mut self, h: Handle<T>) -> Option<&mut T> {
        match self.slots.get_mut(h.index as usize)? {
            Slot::Occupied { generation, value } if *generation == h.generation => Some(value),
            _ => None,
        }
    }

    /// Removes and returns the value behind `h`, invalidating the handle.
    pub fn remove(&mut self, h: Handle<T>) -> Option<T> {
        let slot = self.slots.get_mut(h.index as usize)?;
        match slot {
            Slot::Occupied { generation, .. } if *generation == h.generation => {}
            _ => return None,
        }
        let next_generation = h.generation.wrapping_add(1);
        let old = core::mem::replace(
            slot,
            Slot::Free {
                generation: next_generation,
                next_free: self.free_head,
            },
        );
        self.free_head = Some(h.index);
        self.len -= 1;
        match old {
            Slot::Occupied { value, .. } => Some(value),
            Slot::Free { .. } => None,
        }
    }

    /// Iterates over live values with their handles, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| match s {
            Slot::Occupied { generation, value } => Some((
                Handle {
                    index: i as u32,
                    generation: *generation,
                    _marker: PhantomData,
                },
                value,
            )),
            Slot::Free { .. } => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_handles_do_not_resolve() {
        let mut a = Arena::new();
        let h1 = a.insert("one").unwrap();
        assert_eq!(a.remove(h1), Some("one"));
        let h2 = a.insert("two").unwrap();
        assert_eq!(a.get(h1), None);
        assert_eq!(a.get(h2), Some(&"two"));
        assert_eq!(a.remove(h1), None);
        assert_eq!(a.len(), 1);
    }

    #[test]
    fn free_list_reuses_slots_and_iterates_live_only() {
        let mut a = Arena::new();
        let hs: Vec<_> = (0..4).map(|i| a.insert(i).unwrap()).collect();
        a.remove(hs[1]);
        a.remove(hs[2]);
        let h = a.insert(99).unwrap();
        assert_eq!(a.len(), 3);
        let live: Vec<i32> = a.iter().map(|(_, v)| *v).collect();
        assert_eq!(live.len(), 3);
        assert!(live.contains(&99) && live.contains(&0) && live.contains(&3));
        *a.get_mut(h).unwrap() += 1;
        assert_eq!(a.get(h), Some(&100));
    }
}
