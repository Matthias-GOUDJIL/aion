#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <unistd.h>
#include <string.h>
#include <time.h>
#include <math.h>
#include <sys/wait.h>
#if defined(AION_USE_PTHREAD)
#include <pthread.h>
#else
#include <sched.h>
#include <sys/syscall.h>
#include <sys/mman.h>
#include <linux/futex.h>
#endif
#define GC_THREADS
#define GC_NO_THREAD_REDIRECTS
#include <gc.h>

#if defined(AION_USE_PTHREAD)
typedef pthread_mutex_t aion_mutex_t;
#define AION_MUTEX_INIT PTHREAD_MUTEX_INITIALIZER
static void aion_mutex_lock(aion_mutex_t* m) { pthread_mutex_lock(m); }
static void aion_mutex_unlock(aion_mutex_t* m) { pthread_mutex_unlock(m); }
#else
typedef struct { int state; } aion_mutex_t;
#define AION_MUTEX_INIT { 0 }
static void aion_mutex_lock(aion_mutex_t* m) {
    int expected = 0;
    if (__atomic_compare_exchange_n(&m->state, &expected, 1, 0, __ATOMIC_ACQUIRE, __ATOMIC_RELAXED)) {
        return;
    }
    while (__atomic_exchange_n(&m->state, 2, __ATOMIC_ACQUIRE) != 0) {
        syscall(SYS_futex, &m->state, FUTEX_WAIT_PRIVATE, 2, NULL, NULL, 0);
    }
}
static void aion_mutex_unlock(aion_mutex_t* m) {
    if (__atomic_exchange_n(&m->state, 0, __ATOMIC_RELEASE) == 2) {
        syscall(SYS_futex, &m->state, FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0);
    }
}
#endif

static aion_mutex_t aion_gc_lock = AION_MUTEX_INIT;

static void* aion_gc_malloc(size_t size) {
    aion_mutex_lock(&aion_gc_lock);
    void* p = GC_malloc(size);
    aion_mutex_unlock(&aion_gc_lock);
    return p;
}

static void* aion_gc_malloc_atomic(size_t size) {
    aion_mutex_lock(&aion_gc_lock);
    void* p = GC_malloc_atomic(size);
    aion_mutex_unlock(&aion_gc_lock);
    return p;
}

static void* aion_gc_realloc(void* ptr, size_t size) {
    aion_mutex_lock(&aion_gc_lock);
    void* p = GC_realloc(ptr, size);
    aion_mutex_unlock(&aion_gc_lock);
    return p;
}

static char* aion_gc_strdup(const char* s) {
    aion_mutex_lock(&aion_gc_lock);
    char* p = GC_strdup(s);
    aion_mutex_unlock(&aion_gc_lock);
    return p;
}

// Spark registry: spawned threads are recorded so the runtime can join them
// before the process exits (#176, #72). Default Linux backend uses raw
// `clone()` + `SYS_futex` (`CLONE_CHILD_CLEARTID`) without linking `pthread`;
// `-DAION_USE_PTHREAD` preserves the legacy pthread fallback.
#define MAX_SPARKS 256
#define AION_STACK_SIZE (256 * 1024)

#if defined(AION_USE_PTHREAD)
void* spark_entry_point(void* func_ptr) {
    void (*aion_func)() = (void (*)(void))func_ptr;
    struct GC_stack_base sb;
    if (GC_get_stack_base(&sb) == GC_SUCCESS) {
        GC_register_my_thread(&sb);
    }
    aion_func();
    GC_unregister_my_thread();
    return NULL;
}

static pthread_t spark_threads[MAX_SPARKS];
static int spark_joined[MAX_SPARKS];
static int spark_count = 0;
static aion_mutex_t spark_lock = AION_MUTEX_INIT;

long long aion_syscall_clone_thread(void* func_ptr) {
    if (!func_ptr) return -1;
    pthread_t thread;
    if (pthread_create(&thread, NULL, spark_entry_point, func_ptr) != 0) {
        return -1;
    }
    aion_mutex_lock(&spark_lock);
    int idx = -1;
    if (spark_count < MAX_SPARKS) {
        idx = spark_count++;
        spark_threads[idx] = thread;
        spark_joined[idx] = 0;
    }
    aion_mutex_unlock(&spark_lock);
    return (long long)idx;
}

long long aion_syscall_join_thread(long long handle) {
    if (handle < 0 || handle >= MAX_SPARKS) return -1;
    aion_mutex_lock(&spark_lock);
    if (spark_joined[handle]) {
        aion_mutex_unlock(&spark_lock);
        return 0;
    }
    spark_joined[handle] = 1;
    pthread_t t = spark_threads[handle];
    aion_mutex_unlock(&spark_lock);
    pthread_join(t, NULL);
    return 0;
}

long long aion_syscall_gettid(void) {
    return (long long)(uintptr_t)pthread_self();
}

void aion_syscall_sched_yield(void) {
    sched_yield();
}

long long aion_syscall_futex_wait(long long* addr, long long expected) {
    (void)addr;
    (void)expected;
    sched_yield();
    return 0;
}

long long aion_syscall_futex_wake(long long* addr, long long count) {
    (void)addr;
    (void)count;
    return 0;
}
#else
typedef struct {
    void (*func)(void);
    void* stack_base;
    size_t stack_size;
    int tid_futex;
    int joined;
} aion_spark_slot_t;

static aion_spark_slot_t spark_slots[MAX_SPARKS];
static int spark_count = 0;
static aion_mutex_t spark_lock = AION_MUTEX_INIT;

static int spark_clone_entry(void* arg) {
    aion_spark_slot_t* slot = (aion_spark_slot_t*)arg;
    if (slot && slot->func) {
        slot->func();
    }
    syscall(SYS_exit, 0);
    return 0;
}

long long aion_syscall_clone_thread(void* func_ptr) {
    if (!func_ptr) return -1;
    aion_mutex_lock(&spark_lock);
    if (spark_count >= MAX_SPARKS) {
        aion_mutex_unlock(&spark_lock);
        void (*fn)(void) = (void (*)(void))func_ptr;
        fn();
        return -1;
    }
    int idx = spark_count++;
    aion_spark_slot_t* slot = &spark_slots[idx];
    slot->func = (void (*)(void))func_ptr;
    slot->stack_size = AION_STACK_SIZE;
    slot->stack_base = mmap(
        NULL,
        slot->stack_size,
        PROT_READ | PROT_WRITE,
        MAP_PRIVATE | MAP_ANONYMOUS | MAP_STACK,
        -1,
        0
    );
    if (slot->stack_base == MAP_FAILED) {
        slot->stack_base = NULL;
        slot->tid_futex = 0;
        slot->joined = 1;
        aion_mutex_unlock(&spark_lock);
        return -1;
    }
    aion_mutex_lock(&aion_gc_lock);
    GC_add_roots(slot->stack_base, (char*)slot->stack_base + slot->stack_size);
    aion_mutex_unlock(&aion_gc_lock);

    slot->tid_futex = 1;
    slot->joined = 0;
    char* stack_top = (char*)slot->stack_base + slot->stack_size;
    stack_top = (char*)((uintptr_t)stack_top & ~(uintptr_t)0xF);
    int flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND
              | CLONE_THREAD | CLONE_SYSVSEM
              | CLONE_PARENT_SETTID | CLONE_CHILD_CLEARTID;
    int tid = clone(
        spark_clone_entry,
        stack_top,
        flags,
        slot,
        &slot->tid_futex,
        NULL,
        &slot->tid_futex
    );
    if (tid == -1) {
        slot->tid_futex = 0;
        slot->joined = 1;
        aion_mutex_lock(&aion_gc_lock);
        GC_remove_roots(slot->stack_base, (char*)slot->stack_base + slot->stack_size);
        aion_mutex_unlock(&aion_gc_lock);
        munmap(slot->stack_base, slot->stack_size);
        slot->stack_base = NULL;
        aion_mutex_unlock(&spark_lock);
        return -1;
    }
    aion_mutex_unlock(&spark_lock);
    return (long long)idx;
}

long long aion_syscall_join_thread(long long handle) {
    if (handle < 0 || handle >= MAX_SPARKS) return -1;
    aion_spark_slot_t* slot = &spark_slots[handle];
    while (1) {
        int cur = __atomic_load_n(&slot->tid_futex, __ATOMIC_ACQUIRE);
        if (cur == 0) break;
        syscall(SYS_futex, &slot->tid_futex, FUTEX_WAIT, cur, NULL, NULL, 0);
    }
    aion_mutex_lock(&spark_lock);
    if (!slot->joined && slot->stack_base) {
        slot->joined = 1;
        aion_mutex_lock(&aion_gc_lock);
        GC_remove_roots(slot->stack_base, (char*)slot->stack_base + slot->stack_size);
        aion_mutex_unlock(&aion_gc_lock);
        munmap(slot->stack_base, slot->stack_size);
        slot->stack_base = NULL;
    }
    aion_mutex_unlock(&spark_lock);
    return 0;
}

long long aion_syscall_gettid(void) {
    return (long long)syscall(SYS_gettid);
}

void aion_syscall_sched_yield(void) {
    syscall(SYS_sched_yield);
}

long long aion_syscall_futex_wait(long long* addr, long long expected) {
    if (!addr) return -1;
    return (long long)syscall(SYS_futex, (int*)addr, FUTEX_WAIT, (int)expected, NULL, NULL, 0);
}

long long aion_syscall_futex_wake(long long* addr, long long count) {
    if (!addr) return -1;
    return (long long)syscall(SYS_futex, (int*)addr, FUTEX_WAKE, (int)count, NULL, NULL, 0);
}
#endif

long long aion_thread_id(void) {
    return aion_syscall_gettid();
}

void aion_thread_yield(void) {
    aion_syscall_sched_yield();
}

void aion_spawn(void* func_ptr) {
    (void)aion_syscall_clone_thread(func_ptr);
}

static void aion_join_all(void) {
    aion_mutex_lock(&spark_lock);
    int n = spark_count;
    aion_mutex_unlock(&spark_lock);
    for (int i = 0; i < n; i++) {
        aion_syscall_join_thread(i);
    }
}

// Called once from the synthesized main (before GC_init): installs the
// exit-time join so spawned sparks always run to completion before exit (#176, #72).
void aion_runtime_init(void) {
#if defined(AION_USE_PTHREAD)
    GC_allow_register_threads();
#endif
    atexit(aion_join_all);
}

/* Runtime bounds-check trap for array indexing. #54. */
void aion_array_oob(long long idx, long long len) {
    fprintf(stderr, "error: array index %lld out of bounds (len %lld)\n", idx, len);
    exit(1);
}

/* Runtime trap for integer division or modulo by zero. #204. */
void aion_div_zero(void) {
    fprintf(stderr, "error: division by zero\n");
    exit(1);
}

void* aion_malloc(size_t size) {
    return aion_gc_malloc(size);
}

void* aion_realloc(void* ptr, size_t size) {
    return aion_gc_realloc(ptr, size);
}

void aion_free(void* ptr) {
    // GC handles freeing
}

void aion_memzero(void* ptr, size_t size) {
    memset(ptr, 0, size);
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

char* aion_io_read_line() {
    char* buf = aion_gc_malloc(1024);
    if (fgets(buf, 1024, stdin)) {
        size_t len = strlen(buf);
        if (len > 0 && buf[len-1] == '\n') buf[len-1] = '\0';
        return buf;
    }
    return NULL;
}

// Command line arguments
extern long aion_argc;
extern char** aion_argv;

long aion_get_argc() {
    return aion_argc;
}

char* aion_get_argv_index(long index) {
    if (index >= 0 && index < aion_argc) {
        return aion_argv[index];
    }
    return NULL;
}

char* aion_getenv(const char* key) {
    char* val = getenv(key);
    if (!val) return NULL;
    return aion_gc_strdup(val); // Copy to GC memory for safety
}

char* aion_int_to_str(long long n) {
    char* buf = aion_gc_malloc_atomic(32);
    snprintf(buf, 32, "%lld", n);
    return buf;
}

char* aion_float_to_str(double f) {
    char* buf = aion_gc_malloc_atomic(64);
    snprintf(buf, 64, "%g", f);
    return buf;
}

double aion_str_to_float(const char* s) {
    if (!s) return 0.0;
    char* end;
    double result = strtod(s, &end);
    if (end == s) return 0.0;
    return result;
}

long long aion_str_eq(const char* s1, const char* s2) {
    if (!s1 || !s2) return s1 == s2;
    return strcmp(s1, s2) == 0;
}

char* aion_str_concat(const char* s1, const char* s2) {
    if (!s1 && !s2) return NULL;
    if (!s1) return aion_gc_strdup(s2);
    if (!s2) return aion_gc_strdup(s1);
    char* buf = aion_gc_malloc_atomic(strlen(s1) + strlen(s2) + 1);
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
    char* buf = aion_gc_malloc_atomic(size + 1);
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

// i64 methods
long long aion_i64_abs(long long x) {
    return x < 0 ? -x : x;
}

long long aion_i64_max(long long a, long long b) {
    return a > b ? a : b;
}

long long aion_i64_min(long long a, long long b) {
    return a < b ? a : b;
}

// String methods
long long aion_string_len(const char* s) {
    return s ? strlen(s) : 0;
}

// AI Tensor Support (#203)
struct AionVector {
    void* ptr;
    long long len;
    long long cap;
};

// Matches LLVM layout of `pub struct Tensor { data: Vector<f64>, shape: Vector<i64>, device: String, requires_grad: bool }`:
// composite fields (`data`, `shape`, `device`) are heap pointers (`ptr`).
struct AionTensor {
    struct AionVector* data;
    struct AionVector* shape;
    char* device;
    long long requires_grad;
};

static struct AionVector* aion_clone_shape(struct AionVector* shape) {
    struct AionVector* s = (struct AionVector*)aion_malloc(sizeof(struct AionVector));
    if (!shape || shape->len <= 0) {
        s->ptr = NULL;
        s->len = 0;
        s->cap = 0;
        return s;
    }
    long long n = shape->len;
    long long* buf = (long long*)aion_gc_malloc_atomic(n * sizeof(long long));
    memcpy(buf, shape->ptr, n * sizeof(long long));
    s->ptr = buf;
    s->len = n;
    s->cap = n;
    return s;
}

struct AionTensor* aion_ai_tensor_zeros(struct AionVector* shape) {
    struct AionTensor* t = (struct AionTensor*)aion_malloc(sizeof(struct AionTensor));
    t->shape = aion_clone_shape(shape);
    t->device = aion_gc_strdup("cpu");
    t->requires_grad = 0;

    long long size = 1;
    if (shape && shape->len > 0 && shape->ptr) {
        long long* shape_ptr = (long long*)shape->ptr;
        for (long long i = 0; i < shape->len; i++) {
            size *= shape_ptr[i];
        }
    } else {
        size = 0;
    }
    if (size < 0) size = 0;

    struct AionVector* d = (struct AionVector*)aion_malloc(sizeof(struct AionVector));
    if (size > 0) {
        double* buf = (double*)aion_gc_malloc_atomic(size * sizeof(double));
        memset(buf, 0, size * sizeof(double));
        d->ptr = buf;
    } else {
        d->ptr = NULL;
    }
    d->len = size;
    d->cap = size;
    t->data = d;

    return t;
}

struct AionTensor* aion_ai_tensor_ones(struct AionVector* shape) {
    struct AionTensor* t = aion_ai_tensor_zeros(shape);
    double* data_ptr = (double*)t->data->ptr;
    for (long long i = 0; i < t->data->len; i++) {
        data_ptr[i] = 1.0;
    }
    return t;
}

struct AionTensor* aion_ai_tensor_rand(struct AionVector* shape) {
    struct AionTensor* t = aion_ai_tensor_zeros(shape);
    double* data_ptr = (double*)t->data->ptr;
    for (long long i = 0; i < t->data->len; i++) {
        data_ptr[i] = (double)rand() / RAND_MAX;
    }
    return t;
}

void aion_ai_tensor_backward(struct AionTensor* t) {
    (void)t;
    printf("Called t.backward()\n");
}

struct AionTensor* aion_ai_tensor_matmul(struct AionTensor* t1, struct AionTensor* t2) {
    if (!t1 || !t2 || !t1->shape || !t2->shape || t1->shape->len != 2 || t2->shape->len != 2) {
        fprintf(stderr, "error: tensor matmul requires 2D tensors\n");
        exit(1);
    }
    long long* s1 = (long long*)t1->shape->ptr;
    long long* s2 = (long long*)t2->shape->ptr;
    long long m = s1[0];
    long long k1 = s1[1];
    long long k2 = s2[0];
    long long n = s2[1];
    if (k1 != k2) {
        fprintf(stderr, "error: tensor matmul shape mismatch ([%lld, %lld] vs [%lld, %lld])\n", m, k1, k2, n);
        exit(1);
    }
    struct AionVector out_shape;
    long long dims[2] = { m, n };
    out_shape.ptr = dims;
    out_shape.len = 2;
    out_shape.cap = 2;
    struct AionTensor* out = aion_ai_tensor_zeros(&out_shape);
    double* a = (double*)t1->data->ptr;
    double* b = (double*)t2->data->ptr;
    double* c = (double*)out->data->ptr;
    for (long long i = 0; i < m; i++) {
        for (long long j = 0; j < n; j++) {
            double sum = 0.0;
            for (long long p = 0; p < k1; p++) {
                sum += a[i * k1 + p] * b[p * n + j];
            }
            c[i * n + j] = sum;
        }
    }
    return out;
}

struct AionTensor* aion_ai_tensor_add(struct AionTensor* t1, struct AionTensor* t2) {
    if (!t1 || !t2 || !t1->data || !t2->data || t1->data->len != t2->data->len) {
        fprintf(stderr, "error: tensor add size mismatch\n");
        exit(1);
    }
    struct AionTensor* out = aion_ai_tensor_zeros(t1->shape);
    double* a = (double*)t1->data->ptr;
    double* b = (double*)t2->data->ptr;
    double* c = (double*)out->data->ptr;
    for (long long i = 0; i < out->data->len; i++) {
        c[i] = a[i] + b[i];
    }
    return out;
}

void aion_ai_tensor_move(struct AionTensor* t, const char* device) {
    if (t && device) {
        t->device = aion_gc_strdup(device);
    }
}

double aion_ai_tensor_get(struct AionTensor* t, long long idx) {
    if (!t || !t->data || idx < 0 || idx >= t->data->len) {
        return 0.0;
    }
    double* d = (double*)t->data->ptr;
    return d[idx];
}

void aion_ai_tensor_set(struct AionTensor* t, long long idx, double val) {
    if (!t || !t->data || idx < 0 || idx >= t->data->len) {
        return;
    }
    double* d = (double*)t->data->ptr;
    d[idx] = val;
}

long long aion_ai_tensor_len(struct AionTensor* t) {
    if (!t || !t->data) return 0;
    return t->data->len;
}

double aion_ai_tensor_sum(struct AionTensor* t) {
    if (!t || !t->data) return 0.0;
    double* d = (double*)t->data->ptr;
    double s = 0.0;
    for (long long i = 0; i < t->data->len; i++) {
        s += d[i];
    }
    return s;
}

char* aion_char_to_str(long long c) {
    char* buf = aion_gc_malloc_atomic(2);
    buf[0] = (char)c;
    buf[1] = 0;
    return buf;
}

long long aion_str_at(const char* s, long long i) {
    if (!s || i < 0 || i >= (long long)strlen(s)) return 0;
    return (unsigned char)s[i];
}

char* aion_str_substr(const char* s, long long start, long long len) {
    if (!s || start < 0 || len < 0) return NULL;
    size_t s_len = strlen(s);
    if ((size_t)start >= s_len) return aion_gc_strdup("");
    if ((size_t)start + (size_t)len > s_len) len = s_len - start;
    
    char* buf = aion_gc_malloc_atomic(len + 1);
    strncpy(buf, s + start, len);
    buf[len] = 0;
    return buf;
}

// Time & Date Support (#199)
long long aion_time_now(void) {
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts) != 0) {
        return 0;
    }
    return (long long)ts.tv_sec * 1000LL + (long long)(ts.tv_nsec / 1000000L);
}

void aion_time_sleep(long long millis) {
    if (millis <= 0) return;
    struct timespec req;
    req.tv_sec = (time_t)(millis / 1000LL);
    req.tv_nsec = (long)((millis % 1000LL) * 1000000L);
    nanosleep(&req, NULL);
}

static void aion_ts_to_tm(long long ts_ms, struct tm* out) {
    long long sec_ll = ts_ms >= 0 ? (ts_ms / 1000LL) : ((ts_ms - 999LL) / 1000LL);
    time_t sec = (time_t)sec_ll;
    memset(out, 0, sizeof(struct tm));
    gmtime_r(&sec, out);
}

long long aion_date_year(long long ts_ms) {
    struct tm tm_val;
    aion_ts_to_tm(ts_ms, &tm_val);
    return (long long)(tm_val.tm_year + 1900);
}

long long aion_date_month(long long ts_ms) {
    struct tm tm_val;
    aion_ts_to_tm(ts_ms, &tm_val);
    return (long long)(tm_val.tm_mon + 1);
}

long long aion_date_day(long long ts_ms) {
    struct tm tm_val;
    aion_ts_to_tm(ts_ms, &tm_val);
    return (long long)tm_val.tm_mday;
}

long long aion_date_hour(long long ts_ms) {
    struct tm tm_val;
    aion_ts_to_tm(ts_ms, &tm_val);
    return (long long)tm_val.tm_hour;
}

long long aion_date_min(long long ts_ms) {
    struct tm tm_val;
    aion_ts_to_tm(ts_ms, &tm_val);
    return (long long)tm_val.tm_min;
}

long long aion_date_sec(long long ts_ms) {
    struct tm tm_val;
    aion_ts_to_tm(ts_ms, &tm_val);
    return (long long)tm_val.tm_sec;
}

// Math & Random Support (#201)
double aion_math_sqrt(double x) {
    return sqrt(x);
}

double aion_math_sin(double x) {
    return sin(x);
}

double aion_math_cos(double x) {
    return cos(x);
}

double aion_math_tan(double x) {
    return tan(x);
}

double aion_math_pow(double base, double exp_val) {
    return pow(base, exp_val);
}

double aion_math_exp(double x) {
    return exp(x);
}

double aion_math_log(double x) {
    return log(x);
}

double aion_math_floor(double x) {
    return floor(x);
}

double aion_math_ceil(double x) {
    return ceil(x);
}

double aion_math_fabs(double x) {
    return fabs(x);
}

static unsigned long long aion_rng_state = 0;
static aion_mutex_t aion_rng_lock = AION_MUTEX_INIT;

long long aion_random_get(void) {
    aion_mutex_lock(&aion_rng_lock);
    if (aion_rng_state == 0) {
        struct timespec ts;
        if (clock_gettime(CLOCK_REALTIME, &ts) == 0) {
            aion_rng_state = ((unsigned long long)ts.tv_sec << 32) ^ (unsigned long long)ts.tv_nsec ^ 0x9E3779B97F4A7C15ULL;
        } else {
            aion_rng_state = 0x9E3779B97F4A7C15ULL;
        }
    }
    aion_rng_state = aion_rng_state * 6364136223846793005ULL + 1442695040888963407ULL;
    unsigned long long out = aion_rng_state;
    aion_mutex_unlock(&aion_rng_lock);
    return (long long)(out & 0x7FFFFFFFFFFFFFFFULL);
}

// OS & Process Support (#202)
void aion_os_exit(long long code) {
    exit((int)code);
}

long long aion_os_cpu_count(void) {
    long n = sysconf(_SC_NPROCESSORS_ONLN);
    return n > 0 ? (long long)n : 1LL;
}

char* aion_os_name(void) {
#if defined(__linux__)
    return aion_gc_strdup("linux");
#elif defined(__APPLE__)
    return aion_gc_strdup("macos");
#elif defined(_WIN32)
    return aion_gc_strdup("windows");
#else
    return aion_gc_strdup("unknown");
#endif
}

long long aion_process_run(const char* cmd) {
    if (!cmd) return -1;
    int status = system(cmd);
    if (status == -1) return -1;
    if (WIFEXITED(status)) {
        return (long long)WEXITSTATUS(status);
    }
    return -1;
}

char* aion_process_output(const char* cmd) {
    if (!cmd) return aion_gc_strdup("");
    FILE* pipe = popen(cmd, "r");
    if (!pipe) return aion_gc_strdup("");
    size_t cap = 256;
    size_t len = 0;
    char* buf = (char*)aion_gc_malloc_atomic(cap);
    char chunk[128];
    size_t n;
    while ((n = fread(chunk, 1, sizeof(chunk), pipe)) > 0) {
        if (len + n + 1 > cap) {
            while (len + n + 1 > cap) {
                cap *= 2;
            }
            char* next = (char*)aion_gc_malloc_atomic(cap);
            if (len > 0) {
                memcpy(next, buf, len);
            }
            buf = next;
        }
        memcpy(buf + len, chunk, n);
        len += n;
    }
    pclose(pipe);
    buf[len] = '\0';
    return buf;
}



