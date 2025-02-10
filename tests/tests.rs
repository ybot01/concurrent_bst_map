use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, SystemTime};
use rand::random;
use rand::distr::{Distribution, StandardUniform};
use tokio::task::JoinHandle;
use rand::random_range;
use rust_map::InsertOrUpdateResult;

fn should_update<T: Ord>(value_1: &T, value_2: &T) -> bool{
    value_2 > value_1
}

fn get_vec_of_key_values<T>(length: usize) -> Vec<T> where StandardUniform: Distribution<T>{
    let mut to_return = Vec::<T>::new();
    for _ in 0..length {to_return.push(random())}
    to_return
}

mod concurrent_tests{
    use rust_map::concurrent::Map;
    use super::*;

    #[test]
    fn length_test(){
        let expected = 10000;
        let map = Map::<32, u64>::new();
        get_vec_of_key_values::<([u8; 32],u64)>(expected).iter()
            .for_each(|x| _ = map.insert_or_update(x.0, x.1));
        assert_eq!(map.len(), expected);
    }

    #[test]
    fn get_size(){
        assert_eq!(size_of::<Map<32, u64>>(), 24);
    }

    #[test]
    fn depth_test(){
        let map = Map::<32, u64>::new();
        let mut array = [0;32];
        map.insert_or_update(array,0);
        array[array.len()-1] = 1;
        map.insert_or_update(array, 0);
        println!("{}", map.depth());
    }
    
    #[test]
    fn remove_test(){
        let expected = 10000;
        let to_insert = get_vec_of_key_values::<([u8; 32],u64)>(expected);
        let map = Map::<32, u64>::new();
        to_insert.iter().for_each(|x| _ = map.insert_or_update(x.0, x.1));
        to_insert.iter().for_each(|x| map.remove(x.0));
        assert!(to_insert.iter().all(|x| map.get(x.0).is_none()));
    }

    #[test]
    fn should_update_test() {
        let map = Map::<32, u64>::new();
        let (key, mut value) = ([0; 32], 0);
        assert_eq!(map.insert_or_update_if(key, value, &should_update), InsertOrUpdateResult::Inserted);
        value += 1;
        assert_eq!(map.insert_or_update_if(key, value, &should_update), InsertOrUpdateResult::Updated);
        value -= 1;
        assert_eq!(map.insert_or_update_if(key, value, &should_update), InsertOrUpdateResult::Neither);
    }

    #[test]
    fn insert_and_get_test() {
        let map = Map::<32, u64>::new();
        _ = map.insert_or_update([0; 32], 1);
        assert!(map.get([0;32]).is_some_and(|x| x == 1));
    }

    #[test]
    fn get_closest_test() {
        let map = Map::<32, u64>::new();
        _ = map.insert_or_update([255; 32], 1);
        _ = map.insert_or_update([254; 32], 1);
        //_ = map.insert_or_update([0; 32], 1);
        _ = map.insert_or_update([1; 32], 1);
        assert!(map.get_or_closest_by_key([0;32], true, true).is_some_and(|x| x.0 == [255;32]));
    }

    #[test]
    fn bench_insert_or_update_if(){
        let map = Map::<32, u64>::new();
        let (key, mut value) = ([0; 32], 0);
        let mut true_count = 0;
        let total = 1000000;
        let start_time = SystemTime::now();
        for _ in 0..total{
            if map.insert_or_update_if(key, value, &should_update) != InsertOrUpdateResult::Neither {true_count += 1};
            value += 1;
        }
        println!("{}", total as f64 / SystemTime::now().duration_since(start_time).unwrap().as_secs_f64());
        assert_eq!(true_count, total);
    }

    static GLOBAL_MAP: Map<32, [u8;32]> = Map::new();

    static TRUE_COUNT: AtomicUsize = AtomicUsize::new(0);

    const NO_THREADS: LazyLock<usize> = LazyLock::new(|| num_cpus::get());
    const TOTAL_PER_THREAD: usize = 100000;

    static USER_LIST: LazyLock<RwLock<Vec<([u8; 32], [u8;32])>>> = LazyLock::new(|| RwLock::new(get_vec_of_key_values((*NO_THREADS)*TOTAL_PER_THREAD)));

    #[test]
    fn bench_multi_thread(){
        println!("no_threads: {}", *NO_THREADS);
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                _ = USER_LIST.read().unwrap().clone();
                let mut threads= Vec::<JoinHandle<Duration>>::new();
                for i in 0..(*NO_THREADS){
                    threads.push(tokio::spawn(async move{
                        let start_index = TOTAL_PER_THREAD * i;
                        let start_time = SystemTime::now();
                        USER_LIST.read().map(|read_lock| {
                            for i in start_index..(start_index+TOTAL_PER_THREAD) {
                                let (key, value) = read_lock[i];
                                if GLOBAL_MAP.insert_or_update(key, value) != InsertOrUpdateResult::Neither{
                                    TRUE_COUNT.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }).unwrap();
                        SystemTime::now().duration_since(start_time).unwrap()
                    }))
                }
                while threads.iter().any(|x| !x.is_finished()) {}
                let mut max_duration = Duration::ZERO;
                let mut duration;
                for i in threads{
                    duration = i.await.unwrap();
                    if duration > max_duration{
                        max_duration = duration;
                    }
                }
                println!("{}", ((*NO_THREADS)*TOTAL_PER_THREAD) as f64 / max_duration.as_secs_f64());
                assert_eq!(TRUE_COUNT.load(Ordering::Relaxed), (*NO_THREADS)*TOTAL_PER_THREAD);
                println!("{} %", GLOBAL_MAP.get_used_percent()*100.0);

                threads = Vec::new();
                let rand_key = USER_LIST.read().unwrap()[random_range(0..USER_LIST.read().unwrap().len())].0;
                for _ in 0..(*NO_THREADS){
                    threads.push(tokio::spawn(async move{
                        let start_time = SystemTime::now();
                        for _ in 0..TOTAL_PER_THREAD{
                            _ = GLOBAL_MAP.get_or_closest_by_key(rand_key, false, true);
                        }
                        SystemTime::now().duration_since(start_time).unwrap()
                    }))
                }
                while threads.iter().any(|x| !x.is_finished()) {}
                max_duration = Duration::ZERO;
                for i in threads{
                    duration = i.await.unwrap();
                    if duration > max_duration{
                        max_duration = duration;
                    }
                }
                println!("{}", ((*NO_THREADS)*TOTAL_PER_THREAD) as f64 / max_duration.as_secs_f64());

                threads = Vec::new();
                for i in 0..(*NO_THREADS){
                    threads.push(tokio::spawn(async move{
                        let start_index = TOTAL_PER_THREAD * i;
                        let start_time = SystemTime::now();
                        USER_LIST.read().map(|read_lock| {
                            for i in start_index..(start_index+TOTAL_PER_THREAD) {
                                GLOBAL_MAP.remove(read_lock[i].0);
                            }
                        }).unwrap();
                        SystemTime::now().duration_since(start_time).unwrap()
                    }))
                }
                while threads.iter().any(|x| !x.is_finished()) {}
                max_duration = Duration::ZERO;
                for i in threads{
                    duration = i.await.unwrap();
                    if duration > max_duration{
                        max_duration = duration;
                    }
                }
                println!("{}", ((*NO_THREADS)*TOTAL_PER_THREAD) as f64 / max_duration.as_secs_f64());
                assert_eq!(GLOBAL_MAP.len(), 0);
            });
    }
}

mod non_concurrent_tests{
    use rust_map::non_concurrent::Map;
    use super::*;

    #[test]
    fn length_test(){
        let expected = 10000;
        let mut map = Map::<32, u64>::new();
        get_vec_of_key_values::<([u8; 32],u64)>(expected).iter()
            .for_each(|x| _ = map.insert_or_update(x.0, x.1));
        assert_eq!(map.len(), expected);
    }

    #[test]
    fn get_size(){
        assert_eq!(size_of::<Map<32, u64>>(), 16);
    }

    #[test]
    fn depth_test(){
        let mut map = Map::<32, u64>::new();
        let mut array = [0;32];
        map.insert_or_update(array,0);
        array[array.len()-1] = 1;
        map.insert_or_update(array, 0);
        println!("max_depth: {}", map.depth());
    }

    #[test]
    fn remove_test(){
        let expected = 10000;
        let to_insert = get_vec_of_key_values::<([u8; 32],u64)>(expected);
        let mut map = Map::<32, u64>::new();
        to_insert.iter().for_each(|x| _ = map.insert_or_update(x.0, x.1));
        to_insert.iter().for_each(|x| map.remove(x.0));
        assert!(to_insert.iter().all(|x| map.get(x.0).is_none()));
    }

    #[test]
    fn should_update_test() {
        let mut map = Map::<32, u64>::new();
        let (key, mut value) = ([0; 32], 0);
        assert_eq!(map.insert_or_update_if(key, value, &should_update), InsertOrUpdateResult::Inserted);
        value += 1;
        assert_eq!(map.insert_or_update_if(key, value, &should_update), InsertOrUpdateResult::Updated);
        value -= 1;
        assert_eq!(map.insert_or_update_if(key, value, &should_update), InsertOrUpdateResult::Neither);
    }

    #[test]
    fn insert_and_get_test() {
        let mut map = Map::<32, u64>::new();
        _ = map.insert_or_update([0; 32], 1);
        assert!(map.get([0;32]).is_some_and(|x| x == 1));
    }

    #[test]
    fn get_closest_test() {
        let mut map = Map::<32, u64>::new();
        _ = map.insert_or_update([255; 32], 1);
        _ = map.insert_or_update([254; 32], 1);
        //_ = map.insert_or_update([0; 32], 1);
        _ = map.insert_or_update([1; 32], 1);
        assert!(map.get_or_closest_by_key([0;32], true, true).is_some_and(|x| x.0 == [255;32]));
    }

    #[test]
    fn get_closest_by_key_leading_zeroes_test(){
        let map = rust_map::concurrent::Map::<32, u64>::new();
        let mut key = [255; 32];
        key[0] = 0;
        _ = map.insert_or_update(key, 1);
        key = [0; 32];
        key[0] = 1;
        key[key.len()-1] = u8::MAX;
        _ = map.insert_or_update(key, 1);
        assert!(map.get_or_closest_by_key_leading_zeroes([1;32], true).is_some_and(|x| x.0 == key));
    }

    #[test]
    fn bench(){
        let mut map = Map::<32, u64>::Empty;
        let mut key = [0; 32];
        let mut true_count = 0;
        let total = 1000000;
        let mut start_time = SystemTime::now();
        for _ in 0..total{
            if map.insert_or_update(key, 0) != InsertOrUpdateResult::Neither {true_count += 1};
            for i in (0..key.len()).rev(){
                if key[i] == u8::MAX {key[i] = 0}
                else {
                    key[i] += 1;
                    break;
                }
            }
        }
        println!("inserts/sec: {}", total as f64 / SystemTime::now().duration_since(start_time).unwrap().as_secs_f64());
        assert_eq!(true_count, total);
        println!("used percent: {} %", map.get_used_percent()*100.0);
        key = [0; 32];
        start_time = SystemTime::now();
        for _ in 0..total{
            map.remove(key);
            for i in (0..key.len()).rev(){
                if key[i] == u8::MAX {key[i] = 0}
                else {
                    key[i] += 1;
                    break;
                }
            }
        }
        println!("removes/sec: {}", total as f64 / SystemTime::now().duration_since(start_time).unwrap().as_secs_f64());
        assert_eq!(map.len(), 0);
    }

}
