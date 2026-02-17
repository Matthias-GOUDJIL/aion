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

void aion_spawn(void (*func)(void)) {
    pthread_t thread;
    if (pthread_create(&thread, NULL, spark_entry_point, (void*)func) != 0) {
        fprintf(stderr, "Error: Failed to spawn Spark\n");
        return;
    }
    pthread_detach(thread);
}

char* aion_read_file(const char* path) {
    FILE* f = fopen(path, "rb");
    if (!f) return NULL;

    fseek(f, 0, SEEK_END);
    long fsize = ftell(f);
    fseek(f, 0, SEEK_SET);

    char* string = malloc(fsize + 1);
    fread(string, fsize, 1, f);
    fclose(f);

    string[fsize] = 0;
    return string;
}

int aion_write_file(const char* path, const char* content) {
    FILE* f = fopen(path, "wb");
    if (!f) return -1;
    size_t written = fwrite(content, 1, strlen(content), f);
    fclose(f);
    return (int)written;
}

int aion_fs_exists(const char* path) {
    FILE* f = fopen(path, "rb");
    if (f) {
        fclose(f);
        return 1;
    }
    return 0;
}

char* aion_getenv(const char* key) {
    return getenv(key);
}

char* aion_get_argv_index(char** argv, int index) {
    return argv[index];
}
