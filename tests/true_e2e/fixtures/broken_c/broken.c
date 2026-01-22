/**
 * RCH E2E Test Fixture - Broken C Code
 *
 * This file contains intentional compilation errors for testing
 * exit code 1 propagation from gcc/clang.
 */

#include <stdio.h>

int main(void) {
    // Intentional error: undeclared variable
    int result = undefined_function(42);

    // Intentional error: type mismatch
    char* str = 42;  // warning that becomes error with -Werror

    // Intentional error: missing semicolon
    printf("This will not compile")

    return result;
}
