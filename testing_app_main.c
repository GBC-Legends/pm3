#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <number>\n", argv[0]);
        return 1;
    }

    int value = atoi(argv[1]);

    printf("%d\n", value);
    fprintf(stderr, "%d\n\n\n", value);

    double wait_time = (double)value;
    clock_t start = clock();

    while (((double)(clock() - start)) / CLOCKS_PER_SEC < wait_time) {
        // busy wait
    }

    return 0;
}
