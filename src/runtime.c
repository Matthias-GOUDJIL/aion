#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <unistd.h>

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
