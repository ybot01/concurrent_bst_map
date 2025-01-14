//#![recursion_limit = "128"] //default is 128, can increase if desired

use std::ops::Sub;
use std::sync::RwLock;

#[derive(Debug)]
struct ConcurrentBSTInternal<K,V>{
    key: K,
    value: V,
    child_nodes: [ConcurrentBSTMap<K,V>; 2]
}

impl<K: Copy + Ord + Sub<Output = K>, V: Copy> ConcurrentBSTInternal<K,V>{
    
    const fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            child_nodes: [const { ConcurrentBSTMap::new() }; 2]
        }
    }
}

#[derive(Debug)]
pub struct ConcurrentBSTMap<K,V>(RwLock<Option<Box<ConcurrentBSTInternal<K,V>>>>);

impl<K: Copy + Ord + Sub<Output = K>, V: Copy> ConcurrentBSTMap<K,V>{

    pub const fn new() -> Self{
        Self(RwLock::new(None))
    }

    fn get_index(target: K, current: K) -> usize{
        if target < current {0} else {1}
    }

    pub fn clear(&self){
        *self.0.write().unwrap() = None
    }

    pub fn len(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => 0,
                Some(node) => {
                    1 + node.child_nodes[0].len() + node.child_nodes[1].len()
                }
            }
        }).unwrap()
    }
    
    pub fn depth(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => 0,
                Some(node) => {
                    1 + node.child_nodes[0].depth().max(node.child_nodes[1].depth())
                }
            }
        }).unwrap()
    }
    
    pub fn get(&self, key: K) -> Option<V>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(node) => {
                    if node.key == key {Some(node.value)}
                    else {node.child_nodes[Self::get_index(key, node.key)].get(key)}
                }
            }
        }).unwrap()
    }

    /*
    
    function findClosestValueInBst(tree, target) {
    let closest = tree.value;
  const traverse = (inputTree) => {
        if (inputTree === null) return;
        if (Math.abs(target - closest) > Math.abs(target - inputTree.value)) {
            closest = inputTree.value;
        }
        // As you can see below this line you are checking target < tree.value
        // problem is that tree is the root that your surrounding function gets
        // not the argument that your recursive function gets.
        // both your condition and your parameter to traverse
        // need to be inputTree, not tree
        if (target < tree.value) {
            console.log('left')
            traverse(inputTree.left)
        } else {
            console.log('right')
            traverse(inputTree.right)
        }
        
    }
    traverse(tree)
    return closest;
}
     */
    
    fn abs_diff<T: Ord + Sub<Output = T>>(item_1: T, item_2: T) -> T{
        if item_2 > item_1 {item_2 - item_1} 
        else {item_1 - item_2}
    }
    
    fn get_or_closest_internal(&self, key: K, closest: K) -> Option<K>{
        self.0.read().map(|read_lock| {
            match &*read_lock {
                None => None,
                Some(node) => {
                    if key == node.key {Some(node.key)}
                    else{
                        node.child_nodes[Self::get_index(key, node.key)].get_or_closest_internal(
                            key,
                            if Self::abs_diff(key, node.key) < Self::abs_diff(key, closest) {node.key} else {closest}
                        )
                    }
                }
            }
        }).unwrap()
    }
    
    pub fn get_or_closest(&self, key: K) -> Option<K>{
        self.0.read().map(|read_lock| {
            match &*read_lock {
                None => None,
                Some(node) => {
                    if key == node.key {Some(node.key)}
                    else{
                        node.child_nodes[Self::get_index(key, node.key)].get_or_closest_internal(key, node.key)
                    }
                }
            }
        }).unwrap()
    }

    pub fn contains_key(&self, key: K) -> bool{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => false,
                Some(node) => {
                    if node.key == key {true}
                    else {node.child_nodes[Self::get_index(key, node.key)].contains_key(key)}
                }
            }
        }).unwrap()
    }

    pub fn insert_or_update(&self, key: K, value: V)  -> bool{
        self.insert_or_update_if(key, value, &|_,_| true)
    }

    pub fn insert_or_update_if(&self, key: K, value: V, should_update: &impl Fn(&V, &V) -> bool) -> bool{
        loop{
            match self.0.read().map(|read_lock| {
                match &*read_lock{
                    None => None,
                    Some(node) => {
                        if node.key != key {Some(node.child_nodes[Self::get_index(key, node.key)].insert_or_update_if(key, value, should_update))}
                        else {None}
                    }
                }
            }).unwrap(){
                None => (),
                Some(result) => return result
            }
            match self.0.write().map(|mut write_lock| {
                match &mut *write_lock{
                    None => {
                        //insert
                        *write_lock = Some(Box::new(ConcurrentBSTInternal::new(key, value)));
                        Some(true)
                    }
                    Some(node) => {
                        //if a different key than before then retry the read lock
                        if node.key != key {None}
                        else{
                            //update
                            Some(
                                if should_update(&node.value, &value){
                                    node.value = value;
                                    true
                                }
                                else {false}
                            )
                        }
                    }
                }
            }).unwrap(){
                None => (),
                Some(result) => return result
            }
        }
    }
    
    fn internal_get_replacement_key_value(&self, go_left: bool) -> Option<(K,V)>{
        self.0.write().map(|mut write_lock| {
            match &mut *write_lock {
                None => None,
                Some(node) => {
                    match node.child_nodes[if go_left {0} else {1}].internal_get_replacement_key_value(go_left){
                        None => {
                            //found replacement node with no node in chosen direction
                            let key_value = (node.key, node.value);
                            //if got opposite direction node, recursively run on that
                            match node.child_nodes[if go_left {1} else {0}].internal_get_replacement_key_value(go_left){
                                None => *write_lock = None,
                                Some(result) => (node.key, node.value) = result
                            }
                            Some(key_value)
                        }
                        Some(result) => Some(result)
                    }
                }
            }
        }).unwrap()
    }
    
    pub fn remove_if(&self, key: K, should_remove: &impl Fn(&V) -> bool){
        loop{
            if self.0.read().map(|read_lock| {
                match &*read_lock{
                    None => true,
                    Some(node) => {
                        if node.key != key {
                            node.child_nodes[Self::get_index(key, node.key)].remove_if(key, should_remove);
                            true
                        }
                        else {false}
                    }
                }
            }).unwrap() {return}
            if self.0.write().map(|mut write_lock| {
                match &mut *write_lock{
                    None => true,
                    Some(node) => {
                        //if a different key than before then retry the read lock
                        if node.key != key {false}
                        else if should_remove(&node.value){
                            match node.child_nodes[1].internal_get_replacement_key_value(true)
                                .or(node.child_nodes[0].internal_get_replacement_key_value(false)) {
                                None => *write_lock = None,
                                Some(result) => (node.key, node.value) = result
                            }
                            true
                        }
                        else {true}
                    }
                }
            }).unwrap() {return}
        }
    }

    pub fn remove(&self, key: K){
        self.remove_if(key, &|_| true)
    }

}

#[derive(Debug)]
pub struct ConcurrentBSTSet<K>(ConcurrentBSTMap<K, ()>);

impl<K: Copy + Ord + Sub<Output = K>> ConcurrentBSTSet<K>{
    
    pub const fn new() -> Self{
        Self(ConcurrentBSTMap::new())
    }

    pub fn len(&self) -> usize{
        self.0.len()
    }

    pub fn clear(&self){
        self.0.clear();
    }

    pub fn contains_key(&self, key: K) -> bool{
        self.0.contains_key(key)
    }

    pub fn insert(&self, key: K){
        self.0.insert_or_update_if(key, (), &|_,_| false);
    }
    
    pub fn remove(&self, key: K){
        self.0.remove(key)
    }
}