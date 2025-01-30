A concurrent map written in rust with generic value type and byte array key\
Also a concurrent set which is a wrapper around the map
Implemented using recursive RwLocks, read locks unless absolutely neccesary to write lock to maximise multi thread performance\
Currently the value must implement Copy\
Rule is to minimise dependencies, currently only has single dependency for parking lot to use their rwlock as is 8 bytes Vs std rwlock 16 bytes so reduces overhead

Motivation is I need a multi threaded data structure of key-values in which it is fast to find the key that is equal or closest to a given key\
Was previously using Dashmap library (https://github.com/xacrimon/dashmap) in which it is fast ~O(1) to find a key but best case O(N) time to find nearest key\
With this map should be best case O(log(N)) for both

I'd appreciate pull requests or suggestions for improvement, especially for how to reduce the overhead. for 32 byte key and 32 bytes value overhead is currently around 50%
