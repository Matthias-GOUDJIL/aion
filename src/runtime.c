#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <unistd.h>
#include <string.h>
#include <gc.h>

void* spark_entry_point(void* func_ptr) {
    void (*aion_func)() = (void (*)(void))func_ptr;
    aion_func();
    return NULL;
}

void aion_spawn(void* func_ptr) {
    pthread_t thread;
    pthread_create(&thread, NULL, spark_entry_point, func_ptr);
}

void* aion_malloc(size_t size) {
    return GC_malloc(size);
}

void* aion_realloc(void* ptr, size_t size) {
    return GC_realloc(ptr, size);
}

void aion_free(void* ptr) {
    // GC handles freeing
}

void aion_io_print(const char* msg) {
    if (msg) printf("%s", msg);
    fflush(stdout);
}

void aion_io_println(const char* msg) {
    if (msg) printf("%s\n", msg);
    else printf("\n");
    fflush(stdout);
}

char* aion_int_to_str(long long n) {
    char* buf = GC_malloc_atomic(32);
    snprintf(buf, 32, "%lld", n);
    return buf;
}

char* aion_float_to_str(double f) {
    char* buf = GC_malloc_atomic(64);
    snprintf(buf, 64, "%g", f);
    return buf;
}

long long aion_str_eq(const char* s1, const char* s2) {
    if (!s1 || !s2) return s1 == s2;
    return strcmp(s1, s2) == 0;
}

char* aion_str_concat(const char* s1, const char* s2) {
    if (!s1 && !s2) return NULL;
    if (!s1) return GC_strdup(s2);
    if (!s2) return GC_strdup(s1);
    char* buf = GC_malloc_atomic(strlen(s1) + strlen(s2) + 1);
    strcpy(buf, s1);
    strcat(buf, s2);
    return buf;
}

char* aion_read_file(const char* path) {
    FILE* f = fopen(path, "r");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = GC_malloc_atomic(size + 1);
    fread(buf, 1, size, f);
    buf[size] = 0;
    fclose(f);
    return buf;
}

int aion_write_file(const char* path, const char* content) {
    FILE* f = fopen(path, "w");
    if (!f) return -1;
    int res = fprintf(f, "%s", content);
    fclose(f);
    return res;
}

int aion_append_file(const char* path, const char* content) {
    FILE* f = fopen(path, "a");
    if (!f) return -1;
    int res = fprintf(f, "%s", content);
    fclose(f);
    return res;
}

long long aion_fs_exists(const char* path) {
    return access(path, F_OK) == 0;
}

char* aion_getenv(const char* key) {
    char* val = getenv(key);
    if (!val) return NULL;
    return GC_strdup(val); // Copy to GC memory
}

// AI Tensor Support
struct AionVector {
    void* ptr;
    long long len;
    long long cap;
};

struct AionTensor {
    struct AionVector data;
    struct AionVector shape;
    char* device;
    long long requires_grad;
};

struct AionTensor aion_ai_tensor_zeros(struct AionVector* shape) {
    struct AionTensor t;
    t.shape = *shape; 
    t.device = GC_strdup("cpu");
    t.requires_grad = 0;
    
    long long size = 1;
    long long* shape_ptr = (long long*)shape->ptr;
    for (long long i = 0; i < shape->len; i++) {
        size *= shape_ptr[i];
    }
    
    t.data.ptr = GC_malloc_atomic(size * sizeof(double));
    t.data.len = size;
    t.data.cap = size;
    
    return t;
}

struct AionTensor aion_ai_tensor_ones(struct AionVector* shape) {
    struct AionTensor t = aion_ai_tensor_zeros(shape);
    double* data_ptr = (double*)t.data.ptr;
    for (long long i = 0; i < t.data.len; i++) {
        data_ptr[i] = 1.0;
    }
    return t;
}

struct AionTensor aion_ai_tensor_rand(struct AionVector* shape) {
    struct AionTensor t = aion_ai_tensor_zeros(shape);
    double* data_ptr = (double*)t.data.ptr;
    for (long long i = 0; i < t.data.len; i++) {
        data_ptr[i] = (double)rand() / RAND_MAX;
    }
    return t;
}

void aion_ai_tensor_backward(struct AionTensor* t) {
    // Placeholder for autograd
}

struct AionTensor aion_ai_tensor_matmul(struct AionTensor* t1, struct AionTensor* t2) {
    // Placeholder: return zeros for now
    return aion_ai_tensor_zeros(&t1->shape);
}

struct AionTensor aion_ai_tensor_add(struct AionTensor* t1, struct AionTensor* t2) {
    // Placeholder: return zeros for now
    return aion_ai_tensor_zeros(&t1->shape);
}

void aion_ai_tensor_move(struct AionTensor* t, const char* device) {
    t->device = GC_strdup(device);
}

char* aion_char_to_str(long long c) {
    char* buf = GC_malloc_atomic(2);
    buf[0] = (char)c;
    buf[1] = 0;
    return buf;
}

long long aion_str_at(const char* s, long long i) {
    if (!s || i < 0 || i >= strlen(s)) return 0;
    return (unsigned char)s[i];
}

char* aion_str_substr(const char* s, long long start, long long len) {
    if (!s || start < 0 || len < 0) return NULL;
    size_t s_len = strlen(s);
    if (start >= s_len) return GC_strdup("");
    if (start + len > s_len) len = s_len - start;
    
    char* buf = GC_malloc_atomic(len + 1);
    strncpy(buf, s + start, len);
    buf[len] = 0;
    return buf;
}

char* aion_get_argv_index(char** argv, long long index) {
    if (!argv) return NULL;
    return GC_strdup(argv[index]);
}
