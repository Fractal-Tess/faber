#include <stdio.h>

int main(void) {
    int found = 0;
    for (int n = 2; found < 10; n++) {
        int prime = 1;
        for (int d = 2; d * d <= n; d++)
            if (n % d == 0) { prime = 0; break; }
        if (prime) { printf("%d ", n); found++; }
    }
    putchar('\n');
    return 0;
}
