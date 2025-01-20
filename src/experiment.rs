use std::sync::RwLock;

pub struct ConcurrentMap<const N: usize, V>(RwLock<Option<Box<ConcurrentMapInternal<N, V>>>>);

enum ConcurrentMapInternal<const N: usize, V>{
    Item([u8; N], V),
    List([ConcurrentMap<N, V>; 4])
}

//impl<const N: usize, V: Copy> ConcurrentMapInternal<N, V> {}

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{
    
    const fn get_index(key: [u8; N], depth: usize) -> usize{
        (match depth % 4{
            0 => (key[depth/4] & 0b11000000) >> 6,
            1 => (key[depth/4] & 0b00110000) >> 4,
            2 => (key[depth/4] & 0b00001100) >> 2,
            _ => key[depth/4] & 0b00000011
        }) as usize
    }
    
    pub fn len(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => 0,
                Some(inner) => {
                    match inner.as_ref(){
                        ConcurrentMapInternal::Item(_,_) => 1,
                        ConcurrentMapInternal::List(list) => list.iter().map(|x| x.len()).sum()
                    }
                }
            }
        }).unwrap()
    }
    
    pub const fn new() -> Self{
        Self(RwLock::new(None))
    }

    pub fn clear(&self){
        *self.0.write().unwrap() = None;
    }

    pub fn get(&self, key: [u8; N]) -> Option<V>{
        self.get_internal(key, 0)
    }
    
    fn get_internal(&self, key: [u8; N], depth: usize) -> Option<V>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(inner) => {
                    match inner.as_ref(){
                        ConcurrentMapInternal::Item(item_key, item_value) => if *item_key == key {Some(*item_value)} else {None}
                        ConcurrentMapInternal::List(list) => list[Self::get_index(key, depth)].get_internal(key, depth + 1)
                    }
                }
            }
        }).unwrap()
    }
    
    /*pub fn get_or_closest_by_key(&self, key: [u8; N], include_key: bool) -> Option<([u8; N], V)>{
        self.get_or_closest_by_key_internal(key, include_key, 0, None)
    }

    fn get_or_closest_by_key_internal(&self, key: [u8; N], include_key: bool, depth: usize, closest: Option<([u8; N], V)>) -> Option<([u8; N], V)>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(inner) => {
                    match inner.as_ref(){
                        ConcurrentMapInternal::Item(item_key, item_value) => { 
                            
                        }
                        ConcurrentMapInternal::List(list) => list[Self::get_index(key, depth)].get_internal(key, depth + 1)
                    }
                }
            }
        }).unwrap();
    }*/
    
    pub fn get_min(&self) -> Option<([u8; N], V)>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(inner) => {
                    match inner.as_ref(){
                        ConcurrentMapInternal::Item(item_key, item_value) => Some((*item_key, *item_value)),
                        ConcurrentMapInternal::List(list) => list.iter().find_map(|x| x.get_min())
                    }
                }
            }
        }).unwrap()
    }

    pub fn get_max(&self) -> Option<([u8; N], V)>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(inner) => {
                    match inner.as_ref(){
                        ConcurrentMapInternal::Item(item_key, item_value) => Some((*item_key, *item_value)),
                        ConcurrentMapInternal::List(list) => list.iter().rev().find_map(|x| x.get_max())
                    }
                }
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
        loop{
            match self.0.read().map(|read_lock| {
                match &*read_lock{
                    None => None, //change to write lock
                    Some(inner) => {
                        match inner.as_ref(){
                            ConcurrentMapInternal::Item(_,_) => None, //change to write_lock
                            ConcurrentMapInternal::List(list) => Some(list[Self::get_index(key, depth)].insert_or_update_if_internal(key, value, should_update, depth + 1))
                        }
                    }
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
            match self.0.write().map(|mut write_lock| {
                match &mut *write_lock {
                    None => {
                        //insert
                        *write_lock = Some(Box::new(ConcurrentMapInternal::Item(key, value)));
                        Some(true)
                    }
                    Some(inner) => {
                        match inner.as_mut(){
                            ConcurrentMapInternal::Item(item_key,item_value) => {
                                if *item_key == key{
                                    //update
                                    Some(
                                        if should_update(item_value, &value){
                                            *item_value = value;
                                            true
                                        }
                                        else {false}
                                    )
                                }
                                else{
                                    //insert and restructure
                                    *inner = Box::new(Self::deepen_tree((*item_key, *item_value), (key, value), depth));
                                    Some(true)
                                }
                            }
                            ConcurrentMapInternal::List(_) => None //change back to read lock
                        }
                    }
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
        }
    }

    fn deepen_tree(item_1: ([u8; N], V), item_2: ([u8; N], V), depth: usize) -> ConcurrentMapInternal<N, V> {
        let item_1_index = Self::get_index(item_1.0, depth);
        let item_2_index = Self::get_index(item_2.0, depth);
        let new_list = [const {Self::new()}; 4];
        if item_1_index == item_2_index {
            *new_list[item_1_index].0.write().unwrap() = Some(Box::new(Self::deepen_tree(item_1, item_2, depth + 1)));
        }
        else{
            *new_list[item_1_index].0.write().unwrap() = Some(Box::new(ConcurrentMapInternal::Item(item_1.0, item_1.1)));
            *new_list[item_2_index].0.write().unwrap() = Some(Box::new(ConcurrentMapInternal::Item(item_2.0, item_2.1)));
        }
        ConcurrentMapInternal::List(new_list)
    }

    /*pub fn remove(&self, key: [u8; N], value: V){
        self.remove_if(key, value, &|_,_| true)
    }

    pub fn remove_if(&self, key: [u8; N], value: V, should_remove: &impl Fn(&[u8; N], &V) -> bool){
        self.remove_if_internal(key, value, should_remove, 0);
    }*/

    fn remove_if_internal(&self, key: [u8; N], value: V, should_remove: &impl Fn(&[u8; N], &V) -> bool, depth: usize) -> bool{
        loop {
            match self.0.read().map(|read_lock| {
                match &*read_lock{
                    None => Some(false),
                    Some(inner) => {
                        match inner.as_ref(){
                            ConcurrentMapInternal::Item(_,_) => None, //change to read lock
                            ConcurrentMapInternal::List(list) => Some(list[Self::get_index(key, depth)].remove_if_internal(key, value, should_remove, depth + 1))
                        }
                    }
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
            match self.0.write().map(|mut write_lock| {
                match &mut *write_lock {
                    None => Some(false),
                    Some(inner) => {
                        match inner.as_mut(){
                            ConcurrentMapInternal::Item(item_key,item_value) => {
                                if *item_key == key{
                                    
                                }
                                else{
                                    
                                }
                            }
                            ConcurrentMapInternal::List(_) => None //change back to read lock
                        }
                    }
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
        }
    }
}