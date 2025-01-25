use std::sync::RwLock;

#[derive(Debug)]
pub struct ConcurrentMap<const N: usize, V>(RwLock<Option<ConcurrentMapInternal<N, V>>>);

#[derive(Debug)]
enum ConcurrentMapInternal<const N: usize, V>{
    Item(Box<([u8; N], V)>),
    List(Box<[ConcurrentMap<N, V>; 4]>)
}

impl<const N: usize, V: Copy> ConcurrentMapInternal<N, V> {
    fn new_item(key: [u8; N], value: V) -> Self{
        Self::Item(Box::new((key, value)))
    }
}

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{
    
    const fn get_index(key: [u8; N], depth: usize) -> usize{
         (match depth % 4{
            0 => (key[depth/4] & 0b11000000) >> 6,
            1 => (key[depth/4] & 0b00110000) >> 4,
            2 => (key[depth/4] & 0b00001100) >> 2,
            _ => key[depth/4] & 0b00000011
        }) as usize
    }

    pub fn is_empty(&self) -> bool{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => true,
                Some(inner) => {
                    match inner{
                        ConcurrentMapInternal::Item(_) => false,
                        ConcurrentMapInternal::List(list) => list.iter().all(|x| x.is_empty())
                    }
                }
            }
        }).unwrap()
    }

    pub fn depth(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => 0,
                Some(inner) => {
                    match inner{
                        ConcurrentMapInternal::Item(_) => 1,
                        ConcurrentMapInternal::List(list) => 1 + list.iter().map(|x| x.depth()).max().unwrap()
                    }
                }
            }
        }).unwrap()
    }

    pub fn len(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => 0,
                Some(inner) => {
                    match inner{
                        ConcurrentMapInternal::Item(_) => 1,
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
                    match inner{
                        ConcurrentMapInternal::Item(item) => if item.0 == key {Some(item.1)} else {None}
                        ConcurrentMapInternal::List(list) => list[Self::get_index(key, depth)].get_internal(key, depth + 1)
                    }
                }
            }
        }).unwrap()
    }
    
    /*pub fn get_or_closest_by_key(&self, key: [u8; N], include_key: bool) -> Option<([u8; N], V)>{
        self.get_or_closest_by_key_internal(key, include_key, 0, None).map(|x| x.0)
    }

    fn get_or_closest_by_key_internal(&self, key: [u8; N], include_key: bool, depth: usize, closest: Option<([u8; N], V)>) -> Option<(([u8; N], V), bool, bool)>{
        //go down to where key would be
        //if key is there and include key is true, return
        //if above is false then go up and down the right hand side of left index in list and left hand side of right index
        //if no left or right index then need to go up until there is one
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(inner) => {
                    match inner{
                        ConcurrentMapInternal::Item(item_key_value) => {
                            if (item_key_value.0 != key) || include_key {Some(item_key_value)} else {None}
                        }
                        ConcurrentMapInternal::List(list) => {
                            let index = Self::get_index(key, depth);
                            let result = list[index].get_or_closest_by_key_internal(key, include_key, depth + 1, closest);
                            match 
                            //when index is some then check if 

                            [
                                if index > 0 {list[index-1].get_or_closest_by_key_internal(key, include_key, depth + 1, closest)} else {None},
                                if index < 3 {list[index+1].get_or_closest_by_key_internal(key, include_key, depth + 1, closest)} else {None},
                            ].iter().filter_map(|x| *x).min_by_key(|x| Self::get_abs_diff(key, x.0.0))
                        }
                    }
                }
            }
        }).unwrap();
    }*/

    const HALF_POINT: [u8; N] = {
        let mut array = [0; N];
        array[0] = 1;
        array
    };

    fn get_abs_diff(item_1: [u8; N], item_2: [u8; N]) -> [u8; N]{
        let inner_function = |item_1_inner: [u8; N], item_2_inner: [u8; N]| {
            let mut result = [0; N];
            let mut borrow = 0;
            for i in (0..N).rev() {
                if item_1_inner[i] > item_2_inner[i]{
                    result[i] = item_1_inner[i] - item_2_inner[i] - borrow;
                    borrow = 0;
                }
                else if item_1_inner[i] == item_2_inner[i]{
                    if borrow == 1 {result[i] = u8::MAX}
                    else {result[i] = 0}
                }
                else{
                    result[i] = u8::MAX - (item_2_inner[i] - item_1_inner[i]) + 1 - borrow;
                    borrow = 1;
                }
            }
            result
        };
        let diff = inner_function(item_1, item_2);
        if diff > Self::HALF_POINT {inner_function(item_2, item_1)} else {diff}
    }
    
    pub fn get_min(&self) -> Option<([u8; N], V)>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(inner) => {
                    match inner{
                        ConcurrentMapInternal::Item(item_key_value) => Some((item_key_value.0, item_key_value.1)),
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
                    match inner{
                        ConcurrentMapInternal::Item(item_key_value) => Some((item_key_value.0, item_key_value.1)),
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
                        match inner{
                            ConcurrentMapInternal::Item(_) => None, //change to write_lock
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
                        *write_lock = Some(ConcurrentMapInternal::new_item(key, value));
                        Some(true)
                    }
                    Some(inner) => {
                        match inner{
                            ConcurrentMapInternal::Item(item_key_value) => {
                                if item_key_value.0 == key{
                                    //update
                                    Some(
                                        if should_update(&item_key_value.1, &value){
                                            item_key_value.1 = value;
                                            true
                                        }
                                        else {false}
                                    )
                                }
                                else{
                                    //insert and restructure
                                    *inner = Self::deepen_tree((item_key_value.0, item_key_value.1), (key, value), depth);
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
            *new_list[item_1_index].0.write().unwrap() = Some(Self::deepen_tree(item_1, item_2, depth + 1));
        }
        else{
            *new_list[item_1_index].0.write().unwrap() = Some(ConcurrentMapInternal::new_item(item_1.0, item_1.1));
            *new_list[item_2_index].0.write().unwrap() = Some(ConcurrentMapInternal::new_item(item_2.0, item_2.1));
        }
        ConcurrentMapInternal::List(Box::new(new_list))
    }

    pub fn remove(&self, key: [u8; N]){
        self.remove_if(key, &|_| true);
    }
    
    pub fn remove_if(&self, key: [u8; N], should_remove: &impl Fn(&V) -> bool){
        self.remove_if_internal(key, should_remove, 0);
    }

    fn remove_if_internal(&self, key: [u8; N], should_remove: &impl Fn(&V) -> bool, depth: usize) -> bool{
        //read locks all way down
        //then on way back up prunes the tree if necessary using write locks until not needed
        if !self.0.read().map(|read_lock| {
            match &*read_lock {
                None => true,
                Some(inner) => {
                    match inner{
                        ConcurrentMapInternal::Item(_) => true,
                        ConcurrentMapInternal::List(list) => list[Self::get_index(key, depth)].remove_if_internal(key, should_remove, depth + 1)
                    }
                }
            }
        }).unwrap() {return false}
        self.0.write().map(|mut write_lock| {
            match &mut *write_lock{
                None => true,
                Some(inner) => {
                    match inner{
                        ConcurrentMapInternal::Item(item_key_value) => {
                            if (item_key_value.0 == key) && should_remove(&item_key_value.1) {*write_lock = None}
                            true
                        }
                        ConcurrentMapInternal::List(list) => {
                            let mut list_found = false;
                            let mut first_item = None;
                            for i in list.iter(){
                                if list_found {break}
                                i.0.read().map(|inner_read_lock| {
                                    match &*inner_read_lock{
                                        None => (),
                                        Some(x) => {
                                            match x{
                                                ConcurrentMapInternal::Item(item_key_value) => first_item.get_or_insert((item_key_value.clone(), 0)).1 += 1,
                                                ConcurrentMapInternal::List(_) => list_found = true
                                            }
                                        }
                                    }
                                }).unwrap();
                            }
                            if list_found {false}
                            else{
                                match first_item{
                                    None => {
                                        *write_lock = None;
                                        true
                                    },
                                    Some(x) => {
                                        if x.1 == 1 {
                                            *write_lock = Some(ConcurrentMapInternal::new_item(x.0.0, x.0.1));
                                            true
                                        }
                                        else {false}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }).unwrap()
    }
}