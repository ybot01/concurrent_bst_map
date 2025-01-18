use std::{array::from_fn, sync::RwLock};

pub trait Subdivision{

    fn get_subdivision(&self, depth: usize) -> usize;
}

pub struct ConcurrentMap<K, V>([RwLock<Box<ConcurrentMapInternal<K, V>>>; 256]);

enum ConcurrentMapInternal<K, V>{
    Item(Option<(K, V)>),
    List(ConcurrentMap<K, V>)
}

impl<K: Copy + Ord + Subdivision, V: Copy> ConcurrentMap<K, V>{

    pub fn new() -> Self{
        Self(from_fn(|_| RwLock::new(Box::new(ConcurrentMapInternal::Item(None)))))
    }

    pub fn clear(&self){
        for i in 0..self.0.len(){
            *self.0[i].write().unwrap() = Box::new(ConcurrentMapInternal::Item(None))
        }
    }

    pub fn get(&self, key: K) -> Option<V>{
        self.get_internal(key, 0)
    }
    
    fn get_internal(&self, key: K, depth: usize) -> Option<V>{
        self.0[key.get_subdivision(depth)].read().map(|read_lock| {
            match read_lock.as_ref(){
                ConcurrentMapInternal::Item(item) => {
                    match item{
                        None => None,
                        Some(x) => if x.0 == key {Some(x.1)} else {None}
                    }
                }
                ConcurrentMapInternal::List(list) => list.get_internal(key, depth + 1)
            }
        }).unwrap()
    }
}