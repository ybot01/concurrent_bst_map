use std::sync::RwLock;

pub struct ConcurrentMap<const N: usize, V>(RwLock<ConcurrentMapInternal<N, V>>);

enum ConcurrentMapInternal<const N: usize, V>{
    Item(Option<([u8; N], V)>),
    List([Box<ConcurrentMap<N, V>>; 2])
}

impl<const N: usize, V: Copy> ConcurrentMapInternal<N, V>{
    
    const EMPTY_ITEM: Self = Self::Item(None);
}

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{

    fn go_left(key: [u8; N], depth: usize) -> bool{
        ((key[depth / 8] >> (depth % 8)) & 1) == 0
    }
    
    fn get_index(key: [u8; N], depth: usize) -> usize{
        ((key[depth / 8] >> (depth % 8)) & 1) as usize
    }
    
    pub const fn new() -> Self{
        Self(RwLock::new(ConcurrentMapInternal::EMPTY_ITEM))
    }
    
    const fn new_with(key_value: ([u8; N], V)) -> Self{
        Self(RwLock::new(ConcurrentMapInternal::Item(Some(key_value))))
    }

    fn new_filled_list(item_1: ([u8; N], V), item_2: ([u8; N], V), depth: usize) -> ConcurrentMapInternal<N, V>{
        let item_1_entry = Box::new(Self::new_with(item_1));
        let item_2_entry = Box::new(Self::new_with(item_2));
        
        ConcurrentMapInternal::List(
            match (Self::go_left(item_1.0, depth), Self::go_left(item_2.0, depth)){
                (false, false) => [Box::new(Self::new()), item_1_entry],
                (false, true) => [item_2_entry, item_1_entry],
                (true, false) => [item_1_entry, item_2_entry],
                (true, true) => [item_1_entry, Box::new(Self::new())]
            }
        )
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
        loop{
            match self.0.read().map(|read_lock| {
                match &*read_lock{
                    ConcurrentMapInternal::Item(_) => None, //change to write lock
                    ConcurrentMapInternal::List(list) => Some(list[Self::get_index(key, depth)].insert_or_update_if_internal(key, value, should_update, depth + 1))
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
            match self.0.write().map(|mut write_lock| {
                match &mut *write_lock {
                    ConcurrentMapInternal::Item(item) => {
                        match item{
                            None => {
                                //insert
                                *item = Some((key, value));
                                Some(true)
                            }
                            Some(existing) => {
                                if existing.0 == key{
                                    //update
                                    Some(
                                        if should_update(&existing.1, &value){
                                            existing.1 = value;
                                            true
                                        }
                                        else {false}
                                    )
                                }
                                else{
                                    //tricky bit
                                    
                                    
                                    
                                    Some(true)
                                }
                            }
                        }
                    }
                    ConcurrentMapInternal::List(_) => None //change back to read lock
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
        }
        
    }
}