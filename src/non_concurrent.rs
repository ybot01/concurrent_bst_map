use crate::{get_index, InsertOrUpdateResult};

#[derive(Debug)]
pub struct Map<const N: usize, V>(MapInternal<N, V>);

#[derive(Debug)]
enum MapInternal<const N: usize, V>{
    Item(Box<([u8; N], V)>),
    List(Box<[Map<N, V>; 4]>),
    Empty
}

impl<const N: usize, V: Copy> MapInternal<N, V> {
    fn new_item(key: [u8; N], value: V) -> Self{
        Self::Item(Box::new((key, value)))
    }
}

impl<const N: usize, V: Copy> Map<N, V>{

    pub fn get_used_percent(&self) -> f64{
        (((size_of::<[u8; N]>() + size_of::<V>()) * self.len()) as f64) / (self.get_memory_size() as f64)
    }

    pub fn get_memory_size(&self) -> usize{
        size_of::<Self>() +
            match &self.0 {
                MapInternal::Item(_) => size_of::<[u8; N]>() + size_of::<V>(),
                MapInternal::List(list) => list.iter().map(|x| x.get_memory_size()).sum(),
                MapInternal::Empty => 0
            }
    }

    pub fn is_empty(&self) -> bool{
        match &self.0{
            MapInternal::Item(_) => false,
            MapInternal::List(list) => list.iter().all(|x| x.is_empty()),
            MapInternal::Empty => true
        }
    }

    pub fn depth(&self) -> usize{
        match &self.0{
            MapInternal::Item(_) => 1,
            MapInternal::List(list) => 1 + list.iter().map(|x| x.depth()).max().unwrap(),
            MapInternal::Empty => 0
        }
    }

    pub fn len(&self) -> usize{
        match &self.0{
            MapInternal::Item(_) => 1,
            MapInternal::List(list) => list.iter().map(|x| x.len()).sum(),
            MapInternal::Empty => 0
        }
    }

    pub const fn new() -> Self{
        Self(MapInternal::Empty)
    }

    pub fn clear(&mut self){ 
        self.0 = MapInternal::Empty;
    }

    pub fn get(&self, key: [u8; N]) -> Option<V>{
        self.get_internal(key, 0)
    }

    fn get_internal(&self, key: [u8; N], depth: usize) -> Option<V>{
        match &self.0{
            MapInternal::Item(item) => if item.0 == key {Some(item.1)} else {None},
            MapInternal::List(list) => list[get_index(key, depth)].get_internal(key, depth + 1),
            MapInternal::Empty => None
        }
    }

    pub fn get_or_closest_by_key_leading_zeroes(&self, key: [u8; N], include_key: bool) -> Option<([u8; N], V)>{
        self.get_or_closest_by_key_leading_zeroes_internal(key, include_key, 0)
    }
    
    fn get_or_closest_by_key_leading_zeroes_internal(&self, key: [u8; N], include_key: bool, depth: usize) -> Option<([u8; N], V)>{
        match &self.0{
            MapInternal::Item(item_key_value) => {
                if (item_key_value.0 != key) || include_key {Some((item_key_value.0, item_key_value.1))} else {None}
            }
            MapInternal::List(list) => {
                let index = get_index(key, depth);
                list[index].get_or_closest_by_key_leading_zeroes_internal(key, include_key, depth + 1)
                .or(
                    match index{
                        0 => [1,2,3],
                        1 => [0,2,3],
                        2 => [3,1,0],
                        _ => [2,1,0]
                    }.iter().find_map(|i| list[*i].get_max())
                )
            }
            MapInternal::Empty => None
        }
    }

    pub fn get_or_closest_by_key(&self, key: [u8; N], include_key: bool, loop_around: bool) -> Option<([u8; N], V)>{
        let (result, found_left, found_right) = self.get_or_closest_by_key_internal(key, include_key, 0, None);
        if !loop_around || (found_left == found_right) {result}
        else if !found_left{
            [result, self.get_max()].iter().filter_map(|x| *x).min_by_key(|x| Self::get_abs_diff(key, x.0))
        }
        else{
            [result, self.get_min()].iter().filter_map(|x| *x).min_by_key(|x| Self::get_abs_diff(key, x.0))
        }
    }

    fn get_or_closest_by_key_internal(&self, key: [u8; N], include_key: bool, depth: usize, closest: Option<([u8; N], V)>) -> (Option<([u8; N], V)>, bool, bool){
        //go down to where key would be
        //if key is there and include key is true, return
        //if above is false then go up and down the right hand side of left index in list and left hand side of right index
        //if no left or right index then need to go up until there is one
        match &self.0{
            MapInternal::Item(item_key_value) => {
                (if (item_key_value.0 != key) || include_key {Some((item_key_value.0, item_key_value.1))} else {None}, false, false)
            }
            MapInternal::List(list) => {
                let index = get_index(key, depth);
                let (mut min, mut left, mut right) = list[index].get_or_closest_by_key_internal(key, include_key, depth + 1, closest);
                if !left{
                    for i in (0..index).rev(){
                        match list[i].get_max(){
                            None => (),
                            Some(left_item_key_value) => {
                                left = true;
                                min = [
                                    min,
                                    Some(left_item_key_value)
                                ].iter().filter_map(|x| *x).min_by_key(|x| Self::get_abs_diff(key, x.0));
                                break;
                            }
                        }
                    }
                }
                if !right{
                    for i in (index+1)..list.len(){
                        match list[i].get_min(){
                            None => (),
                            Some(right_item_key_value) => {
                                right = true;
                                min = [
                                    min,
                                    Some(right_item_key_value)
                                ].iter().filter_map(|x| *x).min_by_key(|x| Self::get_abs_diff(key, x.0));
                                break;
                            }
                        }
                    }
                }
                (min, left, right)
            }
            MapInternal::Empty => (None, false, false)
        }
    }

    const HALF_POINT: [u8; N] = {
        let mut array = [0; N];
        array[0] = 128;
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
        match &self.0{
            MapInternal::Item(item_key_value) => Some((item_key_value.0, item_key_value.1)),
            MapInternal::List(list) => list.iter().find_map(|x| x.get_min()),
            MapInternal::Empty => None
        }
    }

    pub fn get_max(&self) -> Option<([u8; N], V)>{
        match &self.0{
            MapInternal::Item(item_key_value) => Some((item_key_value.0, item_key_value.1)),
            MapInternal::List(list) => list.iter().rev().find_map(|x| x.get_max()),
            MapInternal::Empty => None
        }
    }

    pub fn insert_or_update(&mut self, key: [u8; N], value: V) -> InsertOrUpdateResult{
        self.insert_or_update_if(key, value, &|_,_| true)
    }

    pub fn insert_or_update_if(&mut self, key: [u8; N], value: V, should_update: &impl Fn(&V, &V) -> bool) -> InsertOrUpdateResult{
        self.insert_or_update_if_internal(key, value, should_update, 0)
    }

    fn insert_or_update_if_internal(&mut self, key: [u8; N], value: V, should_update: &impl Fn(&V, &V) -> bool, depth: usize) -> InsertOrUpdateResult{
        match &mut self.0{
            MapInternal::Item(item_key_value) => {
                if item_key_value.0 == key{
                    //update
                    if should_update(&item_key_value.1, &value){
                        item_key_value.1 = value;
                        InsertOrUpdateResult::Updated
                    }
                    else {InsertOrUpdateResult::Neither}
                }
                else{
                    //insert and restructure
                    self.0 = Self::deepen_tree((item_key_value.0, item_key_value.1), (key, value), depth);
                    InsertOrUpdateResult::Inserted
                }
            }
            MapInternal::List(list) => list[get_index(key, depth)].insert_or_update_if_internal(key, value, should_update, depth + 1),
            MapInternal::Empty => {
                self.0 = MapInternal::new_item(key, value);
                InsertOrUpdateResult::Inserted
            }
        }
    }

    fn deepen_tree(item_1: ([u8; N], V), item_2: ([u8; N], V), depth: usize) -> MapInternal<N, V> {
        let item_1_index = get_index(item_1.0, depth);
        let item_2_index = get_index(item_2.0, depth);
        let mut new_list = [const {Self::new()}; 4];
        if item_1_index == item_2_index {
            new_list[item_1_index].0 = Self::deepen_tree(item_1, item_2, depth + 1);
        }
        else{
            new_list[item_1_index].0 = MapInternal::new_item(item_1.0, item_1.1);
            new_list[item_2_index].0 = MapInternal::new_item(item_2.0, item_2.1);
        }
        MapInternal::List(Box::new(new_list))
    }

    pub fn remove(&mut self, key: [u8; N]){
        self.remove_if(key, &|_| true);
    }

    pub fn remove_if(&mut self, key: [u8; N], should_remove: &impl Fn(&V) -> bool) -> bool{
        self.remove_if_internal(key, should_remove, 0)
    }

    fn remove_if_internal(&mut self, key: [u8; N], should_remove: &impl Fn(&V) -> bool, depth: usize) -> bool{
        match &mut self.0{
            MapInternal::Item(item_key_value) => {
                if (item_key_value.0 == key) && should_remove(&item_key_value.1) {
                    self.0 = MapInternal::Empty;
                    true
                }
                else {false}
            }
            MapInternal::List(list) => {
                let removed = list[get_index(key, depth)].remove_if_internal(key, should_remove, depth + 1);
                let mut item_count = 0;
                if list.iter().all(|x| {
                    match x.0{
                        MapInternal::Item(_) => {
                            item_count += 1;
                            true
                        },
                        MapInternal::List(_) => false,
                        MapInternal::Empty => true
                    }
                }) && (item_count <= 1){
                    self.0 = list.iter().find_map(|x| {
                        match &x.0 {
                            MapInternal::Item(item_key_value) => Some(MapInternal::new_item(item_key_value.0, item_key_value.1)),
                            MapInternal::List(_) => None,
                            MapInternal::Empty => None
                        }
                    }).unwrap_or(MapInternal::Empty)
                }
                removed
            },
            MapInternal::Empty => false
        }
    }
}

pub type Set<const N: usize> = Map<N, ()>;