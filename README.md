A generic type concurrent binary search tree map written in rust\
Also a binary search tree set which is wrapper around the map\
Implemented using recursive RwLocks, read locks unless absolutely neccesary to write lock to maximise multi thread performance\
Currently the key (K) must implement Copy, Ord and Sub<Output = K> and value must implement Copy\
Rule is to minimise std dependencies and have NO external dependencies which is currently the case\
Exception is for tests for which external dependencies can be added to dev dependencies

Motivation is i need a multi threaded data structure of key-values in which it is fast to find the key that is equal or closest to a given key\
Was previously using Dashmap library (https://github.com/xacrimon/dashmap) in which it is fast ~O(1) to find a key but best case O(N) time to find nearest key\
With a BST should be best case O(log(N)) for both

I'd appreciate pull requests or suggestions for improvement
