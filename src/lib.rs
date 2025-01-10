use std::{hash::{DefaultHasher, Hash, Hasher}, sync::{Mutex, RwLock}};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
struct ConcurrentBSTNode<K,V>{
    key: K,
    value: V,
    child_nodes: [Option<(bool, K)>; 2]
}

#[derive(Debug)]
struct ConcurrentBSTInternal<K,V>{
    no_elements: Mutex<usize>,
    list: Vec<Mutex<Option<ConcurrentBSTNode<K,V>>>>
}

#[derive(Debug)]
pub struct ConcurrentBST<K,V>{
    inner: RwLock<ConcurrentBSTInternal<K,V>>,
    root_node_key: Mutex<Option<K>>
}

pub trait ShouldUpdate{
    fn should_update_to(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum InsertStatus{
    Updated(bool),
    Inserted,
    RebaseRequired
}

impl<K: Copy + Ord + Eq + Hash, V: ShouldUpdate + Copy> ConcurrentBST<K,V>{

    pub fn new() -> Self{
        Self{
            inner: RwLock::new(ConcurrentBSTInternal{
                no_elements: Mutex::new(0),
                list: Vec::from([const {Mutex::new(None)}; 1024])
            }),
            root_node_key: Mutex::new(None)
        }
    }

    fn get_key_hash(key: K, max_value: usize) -> usize{
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() % (max_value as u64)) as usize
    }

    pub fn add_or_update(&self, key: K, value: V) -> bool{
        let inserted = self.inner.read().map(|rw_lock| {
            let inner_function = |start_key| {
                let list_length = rw_lock.list.len();
                let mut inserted = None;
                let mut current_key = start_key;
                let mut current_key_hash = (current_key, Self::get_key_hash(current_key, list_length));
                let mut break_while_loop;
                let mut counter;
                loop {
                    match inserted {
                        Some(result) => return result,
                        None => {
                            if current_key_hash.0 != current_key{
                                current_key_hash = (current_key,  Self::get_key_hash(current_key, list_length));
                            }
                            break_while_loop = false;
                            counter = 0;
                            while !break_while_loop && inserted.is_none() {
                                rw_lock.list[(current_key_hash.1 + counter) % list_length].lock().map(|mut mutex_lock| {
                                    match *mutex_lock {
                                        None => {
                                            rw_lock.no_elements.lock().map(|mut no_elements| {
                                                if *no_elements >= (list_length / 2){
                                                    inserted = Some(InsertStatus::RebaseRequired);
                                                }
                                                else{
                                                    *mutex_lock = Some(ConcurrentBSTNode {
                                                        key,
                                                        value,
                                                        child_nodes: [None; 2]
                                                    });
                                                    inserted = Some(InsertStatus::Inserted);
                                                    *no_elements += 1;
                                                }
                                            }).unwrap();
                                        }
                                        Some(mut node) => {
                                            if node.key == current_key {
                                                if current_key == key{
                                                    inserted = Some(InsertStatus::Updated(
                                                        if node.value.should_update_to(&value){
                                                            node.value = value;
                                                            true
                                                        }
                                                        else {false}
                                                    ));
                                                }
                                                else{
                                                    match *node.child_nodes[if key < current_key {0} else {1}].get_or_insert((false, key)){
                                                        (true, _) => (),
                                                        (false, child_key) => {
                                                            break_while_loop = true;
                                                            current_key = child_key;
                                                        }
                                                    }
                                                }
                                            }
                                            else {counter += 1}
                                        }
                                    }
                                }).unwrap();
                            }
                        }
                    }
                }
            };

            let mut inner_function_result = None;
            let root_node_key = *self.root_node_key.lock().unwrap().get_or_insert_with(|| {
                inner_function_result = Some(inner_function(key));
                key
            });

            inner_function_result.unwrap_or(inner_function(root_node_key))

        }).unwrap();

        match inserted{
            InsertStatus::Updated(was_updated) => was_updated,
            InsertStatus::Inserted => true,
            InsertStatus::RebaseRequired => {
                //rebase the vec
                self.inner.write().map(|mut rw_lock| {
                    if *rw_lock.no_elements.lock().unwrap() >= (rw_lock.list.len() / 2) {
                        let mut new_vec = Vec::<Mutex<Option<ConcurrentBSTNode<K, V>>>>::new();
                        for _ in 0..(rw_lock.list.len() * 2) { new_vec.push(Mutex::new(None)) }
                        let mut key_hash = 0;
                        let mut counter = 0;
                        for possible_node in rw_lock.list.iter() {
                            possible_node.lock().map(|mutex_lock| {
                                match *mutex_lock {
                                    None => (),
                                    Some(node) => {
                                        key_hash = Self::get_key_hash(node.key, new_vec.len());
                                        counter = 0;
                                        while new_vec[(key_hash + counter) % new_vec.len()].lock().unwrap().is_some() {counter += 1}
                                        *new_vec[(key_hash + counter) % new_vec.len()].lock().unwrap() = Some(node);
                                    }
                                }
                            }).unwrap();
                        }
                        rw_lock.list = new_vec;
                    }
                }).unwrap();

                self.add_or_update(key, value)
            }
        }
    }

    pub fn remove(&self, key: K){

    }

    pub fn remove_if(&self, key: K, should_remove: impl FnOnce(&V) -> bool) -> bool{
        true
    }

}

#[cfg(test)]
mod tests {
    use std::{net::{Ipv6Addr, SocketAddrV6}, sync::LazyLock, time::{Duration, SystemTime, UNIX_EPOCH}};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use ed25519_dalek::{ed25519::SignatureBytes, SecretKey};
    use rand::random;
    use tokio::task::JoinHandle;
    use super::*;

    pub(crate) fn timestamp() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_millis() as u64
    }

    #[derive(Debug, Clone, PartialEq, Eq, Copy)]
    pub(crate) struct User{
        user_id: SecretKey,
        sock_addr: SocketAddrV6,
        update_counter: u64,
        signature: SignatureBytes
    }

    impl ShouldUpdate for User{
        fn should_update_to(&self, other: &Self) -> bool {
            other.update_counter > self.update_counter
        }
    }

    impl User{
        pub fn random() -> Self{
            Self{
                user_id: random(),
                sock_addr: SocketAddrV6::new(Ipv6Addr::from(random::<[u8;16]>()), random(), 0, 0),
                update_counter: timestamp(),
                signature: [0;64]
            }
        }
    }

    #[test]
    fn test() {
        let bst = ConcurrentBST::<SecretKey, User>::new();
        let mut user = User::random();
        assert!(bst.add_or_update(user.user_id, user));
        user.update_counter += 1;
        assert!(bst.add_or_update(user.user_id, user));
        user.update_counter -= 1;
        assert!(!bst.add_or_update(user.user_id, user));
    }

    #[test]
    fn bench(){
        let bst = ConcurrentBST::<SecretKey, User>::new();
        let mut user = User::random();
        let mut true_count = 0;
        let total = 1000000;
        let start_time = SystemTime::now();
        for _ in 0..total{
            if bst.add_or_update(user.user_id, user) {true_count += 1};
            user.update_counter += 1;
        }
        println!("{}", total as f64 / SystemTime::now().duration_since(start_time).unwrap().as_secs_f64());
        assert_eq!(true_count, total);
    }

    static GLOBAL_BST: LazyLock<ConcurrentBST<SecretKey, User>> = LazyLock::new(ConcurrentBST::new);

    const NO_THREADS: usize = 10;

    const TOTAL_PER_THREAD: usize = 10000;

    static TRUE_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn bench_multi_thread(){
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut threads= Vec::<JoinHandle<Duration>>::new();
                for _ in 0..NO_THREADS{
                    threads.push(tokio::spawn(async{
                        let mut random_users = Vec::<User>::new();
                        for _ in 0..TOTAL_PER_THREAD {random_users.push(User::random())}
                        let start_time = SystemTime::now();
                        for user in random_users{
                            if GLOBAL_BST.add_or_update(user.user_id, user){
                                TRUE_COUNT.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        SystemTime::now().duration_since(start_time).unwrap()
                    }))
                }
                while threads.iter().any(|x| !x.is_finished()) {}
                let mut max_duration = Duration::ZERO;
                for i in threads{
                    let duration = i.await.unwrap();
                    if duration > max_duration{
                        max_duration = duration;
                    }
                }
                println!("{}", (NO_THREADS*TOTAL_PER_THREAD) as f64 / max_duration.as_secs_f64());
                //test how many have been filled, is it = no threads * total per thread?
                println!("{:?}", TRUE_COUNT);
            });
    }
}