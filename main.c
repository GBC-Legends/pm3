#include <stdio.h>
#include <time.h>

int main() {
    printf("%d\n", 10);

    double wait_time = 10.0;
    clock_t start = clock();
    while (((double)(clock() - start)) / CLOCKS_PER_SEC < wait_time) {

    }

    return 0;
}
