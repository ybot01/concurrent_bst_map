use std::{array::from_fn, sync::RwLock};

pub struct ConcurrentMap<const N: usize, V>(RwLock<ConcurrentMapInternal<N, V>>);

enum ConcurrentMapInternal<const N: usize, V>{
    Item(Option<([u8; N], V)>),
    List([Box<ConcurrentMap<N, V>>; 2])
}

impl<const N: usize, V> ConcurrentMapInternal<N, V>{
    
    const EMPTY_ITEM: Self = Self::Item(None);
}

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{

    fn get_index(key: [u8; N], depth: usize) -> usize{
        ((key[depth / 8] >> (depth % 8)) & 1) as usize
    }
    
    pub const fn new() -> Self{
        Self(RwLock::new(ConcurrentMapInternal::EMPTY_ITEM))
    }

    fn new_filled_list(item_1: ([u8; N], V), item_2: ([u8; N], V), depth: usize) -> ConcurrentMapInternal<N, V>{
        ConcurrentMapInternal::List(from_fn(|x| {
            Box::new(
                Self(
                    RwLock::new(ConcurrentMapInternal::Item(
                        if Self::get_index(item_1.0, depth) == x {Some(item_1)}
                        else if Self::get_index(item_2.0, depth) == x {Some(item_2)}
                        else {None}
                    ))
                )
            )
        }))
    }

    pub fn clear(&self){
        *self.0.write().unwrap() = ConcurrentMapInternal::EMPTY_ITEM
    }

    pub fn get(&self, key: [u8; N]) -> Option<V>{
        self.get_internal(key, 0)
    }
    
    fn get_internal(&self, key: [u8; N], depth: usize) -> Option<V>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                ConcurrentMapInternal::Item(item) => return item.filter(|x| x.0 == key).map(|x| x.1),
                ConcurrentMapInternal::List(list) => list[Self::get_index(key, depth)].get_internal(key, depth + 1)
            }
        }).unwrap()
    }

    pub fn insert_or_update(&self, key: [u8; N], value: V) -> bool{
        self.insert_or_update_if(key, value, &|_,_| true)
    }
    
    pub fn insert_or_update_if(&self, key: [u8; N], value: V, should_update: &impl Fn(&V, &V) -> bool) -> bool{
        self.insert_or_update_if_internal(key, value, should_update, 0)
    }

    fn insert_or_update_if_internal(&self, key: [u8; N], value: V, should_update: &impl Fn(&V, &V) -> bool, depth: usize) -> bool{
        
    }
}