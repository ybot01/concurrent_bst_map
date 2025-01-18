use std::{array::from_fn, sync::RwLock};

pub struct ConcurrentMap<const N: usize, V>([RwLock<Box<ConcurrentMapInternal<N, V>>>; 2]);

enum ConcurrentMapInternal<const N: usize, V>{
    Item(Option<([u8; N], V)>),
    List(ConcurrentMap<N, V>)
}

impl<const N: usize, V> ConcurrentMapInternal<N, V>{
    
    const EMPTY_ITEM: Self = Self::Item(None);
}

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{

    fn get_index(key: [u8; N], depth: usize) -> usize{
        ((key[depth / 8] >> (depth % 8)) & 1) as usize
    }
    
    pub fn new() -> Self{
        Self(from_fn(|_| RwLock::new(Box::new(ConcurrentMapInternal::EMPTY_ITEM))))
    }

    fn new_filled_list(item_1: ([u8; N], V), item_2: ([u8; N], V), depth: usize) -> ConcurrentMapInternal<N, V>{
        ConcurrentMapInternal::List(
            Self(from_fn(|x| {
                RwLock::new(Box::new(ConcurrentMapInternal::Item({
                    if Self::get_index(item_1.0, depth) == x {Some(item_1)}
                    else if Self::get_index(item_2.0, depth) == x {Some(item_2)}
                    else {None}
                })))
            }))
        )
    }

    pub fn clear(&self){
        for i in 0..self.0.len(){
            *self.0[i].write().unwrap() = Box::new(ConcurrentMapInternal::EMPTY_ITEM)
        }
    }

    pub fn get(&self, key: [u8; N]) -> Option<V>{
        self.get_internal(key, 0)
    }
    
    fn get_internal(&self, key: [u8; N], depth: usize) -> Option<V>{
        self.0[Self::get_index(key, depth)].read().map(|read_lock| {
            match read_lock.as_ref(){
                ConcurrentMapInternal::Item(item) => return item.filter(|x| x.0 == key).map(|x| x.1),
                ConcurrentMapInternal::List(list) => list.get_internal(key, depth + 1)
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
        let index = Self::get_index(key, depth);
        loop{
            match self.0[index].read().map(|read_lock| {
                match read_lock.as_ref(){
                    ConcurrentMapInternal::Item(_) => None, //change to write lock
                    ConcurrentMapInternal::List(list) => Some(list.insert_or_update_if_internal(key, value, should_update, depth + 1))
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
            match self.0[index].write().map(|mut write_lock| {
                match write_lock.as_mut(){
                    ConcurrentMapInternal::Item(item) => {
                        //insert or update
                        match item{
                            Some(existing) => {
                                if existing.0 == key{
                                    Some(
                                        if should_update(&existing.1, &value){
                                            existing.1 = value;
                                            true
                                        }
                                        else {false}
                                    )
                                }
                                else{
                                    //keep going down to next bit until find where the bits are not equal
                                    
                                    Some(true)
                                }
                            }
                            None => {
                                *item = Some((key, value));
                                Some(true)
                            }
                        }
                    }
                    ConcurrentMapInternal::List(_) => None
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
        }
        
    }
}