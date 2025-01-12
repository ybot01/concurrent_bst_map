use std::{hash::{DefaultHasher, Hash, Hasher}, sync::{Mutex, RwLock}};
use std::ops::Deref;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};
use std::sync::atomic::{AtomicUsize, Ordering};

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
    no_elements: AtomicUsize,
    root_node_key: Option<K>,
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
    Inserted(bool)
}

enum LockGuard<'a, K, V> {
    Read(RwLockReadGuard<'a, ConcurrentBSTInternal<K,V>>),
    Write(RwLockWriteGuard<'a, ConcurrentBSTInternal<K,V>>),
}

impl<'a, K, V> Deref for LockGuard<'a, K, V>{
    type Target = ConcurrentBSTInternal<K,V>;

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
                no_elements: AtomicUsize::new(0),
                root_node_key: None,
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
        
        //need to figure out how to do removals using a read lock instead of write lock for efficiency
        //example problem is if user gets the root node key and then tries to find that node
        //could be removed from the bst and replaced by different root node before can read it so will never find it
        //therefore if removing the root node key need to write lock it to ensure exclusive access
        //remove and replace with new or none if no child nodes
        //if removing a none root node then only care about the nodes parent node reference to the node and everything below that so mark that as locked
        //mark as unlocked once removed
        
        let inner_function = |root_node_key, lock_guard: LockGuard<K, V>| {
            let mut current_key = root_node_key;
            let mut current_key_hash;
            let list_length = lock_guard.list.len();
            let mut inserted = None;
            loop {
                match inserted {
                    Some(result) => return result,
                    None => ()
                }
                current_key_hash = Self::get_key_hash(current_key, list_length);
                let mut continue_loop = true;
                while continue_loop && inserted.is_none() {
                    lock_guard.list[current_key_hash].lock().map(|mut mutex_lock| {
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
                                //not found
                                if current_key == key{
                                    mutex_lock.push(ConcurrentBSTNode::new(key, value));
                                    inserted = Some(InsertStatus::Inserted(
                                        lock_guard.no_elements.fetch_add(1, Ordering::Relaxed) >= list_length
                                    ));
                                }
                            }
                        }
                    }).unwrap();
                }
            }
        };
        match self.inner.read().map(|read_lock| 
            read_lock.root_node_key.map(|x| inner_function(x, LockGuard::Read(read_lock)))
        ).unwrap().unwrap_or(
            self.inner.write().map(|mut write_lock| 
                inner_function(*write_lock.root_node_key.get_or_insert(key), LockGuard::Write(write_lock))
            ).unwrap()
        ){
            InsertStatus::Updated(was_updated) => was_updated,
            InsertStatus::Inserted(false) => true,
            InsertStatus::Inserted(true) => {
                self.inner.write().map(|mut rw_lock| {
                    //resize the array if required
                    let mut new_vec_length = rw_lock.list.len();
                    while rw_lock.no_elements.load(Ordering::Relaxed) >= new_vec_length {new_vec_length *= 2}
                    if new_vec_length > rw_lock.list.len(){
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
                true
            }
        }
    }
    
    pub fn get(&self, key: K) -> Option<V>{
        self.inner.read().map(|rw_lock| {
            rw_lock.list[Self::get_key_hash(key, rw_lock.list.len())].lock().unwrap()
            .iter().find(|x| x.key == key).map(|x| x.value)
        }).unwrap()
    }
    
    pub fn remove(&self, key: K){ 
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

        let inner_function = |root_node_key, lock_guard: LockGuard<K, V>| {
            let mut current_key = root_node_key;
            let mut current_key_hash;
            let list_length = lock_guard.list.len();
            let mut removed = false;
            
        };
        
        let removed = self.inner.read().map(|read_lock| {
            match read_lock.root_node_key{
                None => true,
                Some(root_node_key) => {
                    if root_node_key == key{
                        //need to get write lock
                        false
                    }
                    else{
                        inner_function(root_node_key, LockGuard::Read(read_lock));
                        true
                    }
                }
            }
        }).unwrap();
        
        if !removed{
            self.inner.write().map(|write_lock| {
                inner_function(key, LockGuard::Write(write_lock));
            }).unwrap();
        }
    }
    
}