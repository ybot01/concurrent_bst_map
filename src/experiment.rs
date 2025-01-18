use std::{array::from_fn, sync::RwLock};

pub struct ConcurrentMap<const N: usize, V>([RwLock<Box<ConcurrentMapInternal<N, V>>>; 2]);

enum ConcurrentMapInternal<const N: usize, V>{
    Item(Option<([u8; N], V)>),
    List(ConcurrentMap<N, V>)
}

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{

    pub fn new() -> Self{
        Self(from_fn(|_| RwLock::new(Box::new(ConcurrentMapInternal::Item(None)))))
    }

    pub fn clear(&self){
        for i in 0..self.0.len(){
            *self.0[i].write().unwrap() = Box::new(ConcurrentMapInternal::Item(None))
        }
    }

    pub fn get(&self, key: [u8; N]) -> Option<V>{
        self.get_internal(key, 0)
    }
    
    fn get_internal(&self, key: [u8; N], depth: usize) -> Option<V>{
        let modulus = depth % 8;
        let byte = depth / 8;
        
        
        self.0[index as usize].read().map(|read_lock| {
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