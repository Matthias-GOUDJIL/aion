#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <unistd.h>
#include <string.h>

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
    void* p = malloc(size);
    // fprintf(stderr, "aion_malloc(%zu) = %p\n", size, p);
    return p;
}

void* aion_realloc(void* ptr, size_t size) {
    void* p = realloc(ptr, size);
    // fprintf(stderr, "aion_realloc(%p, %zu) = %p\n", ptr, size, p);
    return p;
}

void aion_free(void* ptr) {
    free(ptr);
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
    char* buf = malloc(32);
    snprintf(buf, 32, "%lld", n);
    return buf;
}

char* aion_float_to_str(double f) {
    char* buf = malloc(64);
    snprintf(buf, 64, "%g", f);
    return buf;
}

long long aion_str_eq(const char* s1, const char* s2) {
    if (!s1 || !s2) return s1 == s2;
    return strcmp(s1, s2) == 0;
}

char* aion_read_file(const char* path) {
    FILE* f = fopen(path, "r");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = malloc(size + 1);
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

long long aion_fs_exists(const char* path) {
    return access(path, F_OK) == 0;
}

char* aion_getenv(const char* key) {
    return getenv(key);
}

char* aion_get_argv_index(char** argv, long long index) {
    if (!argv) return NULL;
    return argv[index];
}
