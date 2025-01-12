use std::{hash::{DefaultHasher, Hash, Hasher}, sync::{Mutex, RwLock}};
use std::ops::Deref;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
struct ConcurrentBSTNode<K,V>{
    key: K,
    value: V,
    child_nodes: [Option<(bool, K)>; 2]
}

impl<K: Copy,V> ConcurrentBSTNode<K,V>{
    const fn new(key: K, value: V) -> Self{
        Self{
            key,
            value,
            child_nodes: [None; 2]
        }
    }
}

#[derive(Debug)]
struct ConcurrentBSTInternal<K,V>{
    no_elements: Mutex<usize>,
    root_node_key: Mutex<Option<K>>,
    list: Vec<Mutex<Vec<ConcurrentBSTNode<K,V>>>>
}

#[derive(Debug)]
pub struct ConcurrentBST<K,V>{
    inner: RwLock<ConcurrentBSTInternal<K,V>>
}

pub trait ShouldUpdate{
    fn should_update_to(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum InsertStatus{
    Updated(bool),
    Inserted,
    SizeIncreaseRequired
}

enum LockGuard<'a, K, V> {
    Read(RwLockReadGuard<'a, ConcurrentBSTInternal<K, V>>),
    Write(RwLockWriteGuard<'a, ConcurrentBSTInternal<K, V>>)
}

impl<'a, K,V> Deref for LockGuard<'a, K,V>{
    type Target = ConcurrentBSTInternal<K, V>;
    
    fn deref(&self) -> &Self::Target {
        match self{
            LockGuard::Read(lock) => lock,
            LockGuard::Write(lock) => lock 
        }
    }
}

impl<K: Copy + Ord + Eq + Hash, V: Copy + ShouldUpdate> ConcurrentBST<K,V>{
    
    pub fn new() -> Self{
        Self{
            inner: RwLock::new(ConcurrentBSTInternal{
                no_elements: Mutex::new(0),
                root_node_key: Mutex::new(None),
                list: Vec::from([const {Mutex::new(Vec::new())}; 1024])
            })
        }
    }
    
    fn get_key_hash(key: K, max_value: usize) -> usize{
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() % (max_value as u64)) as usize
    }
    
    pub fn add_or_update(&self, key: K, value: V) -> bool{
        
        match self.inner.read().map(|rw_lock| {
            
            let inner_function = |root_node_key| {
                let mut current_key = root_node_key;
                let mut current_key_hash;
                let list_length = rw_lock.list.len();
                let mut inserted = None;
                loop {
                    match inserted {
                        Some(result) => return result,
                        None => {
                            current_key_hash = Self::get_key_hash(current_key, list_length);
                            let mut continue_loop = true;
                            while continue_loop && inserted.is_none() {
                                rw_lock.list[current_key_hash].lock().map(|mut mutex_lock| {
                                    match mutex_lock.iter().position(|x| x.key == current_key){
                                        Some(index) => {
                                            if current_key == key {
                                                //update
                                                inserted = Some(InsertStatus::Updated(
                                                    if mutex_lock[index].value.should_update_to(&value){
                                                        mutex_lock[index].value = value;
                                                        true
                                                    }
                                                    else {false}
                                                ));
                                            }
                                            else {
                                                //go to next child node if not locked
                                                //if locked exit lock and keep reacquiring until not locked
                                                match *mutex_lock[index].child_nodes[if key < current_key {0} else {1}].get_or_insert((false, key)) {
                                                    (true, _) => (),
                                                    (false, child_key) => {
                                                        continue_loop = false;
                                                        current_key = child_key;
                                                    },
                                                }
                                            }
                                        }
                                        None => {
                                            //not found, insert if enough room
                                            rw_lock.no_elements.lock().map(|mut no_elements| {
                                                inserted = Some(
                                                    if *no_elements >= list_length {InsertStatus::SizeIncreaseRequired}
                                                    else{
                                                        mutex_lock.push(ConcurrentBSTNode::new(key, value));
                                                        *no_elements += 1;
                                                        InsertStatus::Inserted
                                                    }
                                                );
                                            }).unwrap();
                                        }
                                    }
                                }).unwrap();
                            }
                        }
                    }
                }
            };
            
            let mut insert_result = None;
            
            let root_node_key = *rw_lock.root_node_key.lock().unwrap().get_or_insert_with(|| {
                insert_result = Some(inner_function(key));
                key
            });
            
            insert_result.unwrap_or(inner_function(root_node_key))
            
        }).unwrap(){
            InsertStatus::Updated(was_updated) => was_updated,
            InsertStatus::Inserted => true,
            InsertStatus::SizeIncreaseRequired => {
                //double the length of the vec if required
                self.inner.write().map(|mut rw_lock| {
                    if *rw_lock.no_elements.lock().unwrap() >= rw_lock.list.len(){
                        let new_vec_length = rw_lock.list.len() * 2;
                        let mut new_vec = Vec::<Mutex<Vec<ConcurrentBSTNode<K,V>>>>::new();
                        for _ in 0..new_vec_length { new_vec.push(Mutex::new(Vec::new())) }
                        for vec in rw_lock.list.iter(){
                            vec.lock().unwrap().iter().for_each(|node| {
                                new_vec[Self::get_key_hash(node.key, new_vec_length)].lock().unwrap().push(*node);
                            })
                        }
                        rw_lock.list = new_vec;
                    }
                }).unwrap();
                
                self.add_or_update(key, value)
            }
        }
    }
    
    pub fn get(&self, key: K) -> Option<V>{
        self.inner.read().map(|rw_lock| {
            rw_lock.list[Self::get_key_hash(key, rw_lock.list.len())].lock().unwrap()
            .iter().find(|x| x.key == key).map(|x| x.value)
        }).unwrap()
    }
    
    /*pub fn remove(&self, key: K){
    self.remove_if(key, |_| true);
    }
    
    pub fn remove_if(&self, key: K, should_remove: impl Fn(&V) -> bool){
    //need to find the node which has the node to remove as one of its child node references
    //(if reach an empty reference then exit, key is not in map)
    //this is the parent node, lock the child node reference of the parent node that is the key to remove
    //first go to child node which is the key to remove and check if should be removed, if not then unlock the parent and exit
    //then find the next biggest key after the key to remove by going to right child node and then going to left node until reach a leaf node
    //if there is no right child node of key to remove then simply replace the parent child node with the left child node of the key to remove
    //if no child nodes then set the parent child node to empty and unlock
    //if exists then replace the parent child node with this node and set the child nodes of this node to the child nodes of the key to remove
    //after done all this then delete the user
    //if key to delete is the root node then need to lock root node mutex
    //replace the root node with next biggest node if exists and change the child nodes
    //else if only left child node set root node to left node
    //else if no child nodes then set root node to none
    
    let size_decrease_required = self.inner.read().map(|rw_lock| {
    
    let possible_root_node_key =rw_lock.root_node_key.lock().map(|mutex_lock| {
    match *mutex_lock{
    None => None,
    Some(result) => {
    if result == key{
    //todo
    None
    }
    else {Some(result)}
    }
    }
    }).unwrap();
    
    
    match possible_root_node_key{
    None => return false,
    Some(root_node_key) => {
    let list_length = rw_lock.list.len();
    let mut current_key = root_node_key;
    let mut current_key_hash;
    let mut parent_node_internal = None;
    let mut parent_node = loop{
    match parent_node_internal{
    Some(result) => break result,
    None => {
    current_key_hash = Self::get_key_hash(current_key, list_length);
    let mut continue_loop = true;
    while continue_loop && parent_node_internal.is_none(){
    rw_lock.list[current_key_hash].lock().map(|mut mutex_lock| {
    match mutex_lock.iter().position(|x| x.key == current_key){
    None => (), //shouldnt happen
    Some(index) => {
    match &mut mutex_lock[index].child_nodes[if key < current_key {0} else {1}]{
    None => parent_node_internal = Some(None),
    Some((locked, child_key)) => if !*locked{
    if *child_key == key{
    *locked = true;
    parent_node_internal = Some(Some(current_key));
    }
    }
    }
    }
    }
    }).unwrap();
    }
    }
    }
    };
    
    let list_length = rw_lock.list.len();
    let mut is_size_decrease_required = None;
    let mut current_key = root_node_key;
    let mut current_key_hash;
    loop{
    match is_size_decrease_required{
    Some(result) => return result,
    None => {
    current_key_hash = Self::get_key_hash(current_key, list_length);
    let mut continue_loop = true;
    while continue_loop && is_size_decrease_required.is_none(){
    rw_lock.list[current_key_hash].lock().map(|mut mutex_lock| {
    match mutex_lock.iter().position(|x| x.key == current_key){
    None => is_size_decrease_required = Some(false),
    Some(index) => {
    if current_key == key {
    //update
    inserted = Some(InsertStatus::Updated(
    if mutex_lock[index].value.should_update_to(&value){
    mutex_lock[index].value = value;
    true
    }
    else {false}
    ));
    }
    else {
    //go to next child node if not locked
    //if locked exit lock and keep reacquiring until not locked
    match *mutex_lock[index].child_nodes[if key < current_key {0} else {1}].get_or_insert((false, key)) {
    (true, _) => (),
    (false, child_key) => {
    continue_loop = false;
    current_key = child_key;
    },
    }
    }
    }
    }
    }).unwrap();
    }
    }
    }
    }
    }
    }
    
    
    }).unwrap();
    }*/
    
}