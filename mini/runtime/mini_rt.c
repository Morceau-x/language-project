// The Mini runtime: the part of every native Mini program that is not
// compiled from Mini. It reads the inputs from the command line, calls
// the compiled code, and prints the result.
//
// `mini build` compiles it together with the LLVM IR from the compiler:
//
//     clang -O2 program.ll mini_rt.c -o program
//
// Real languages ship the same kind of thing: C has crt0 and libc, Rust
// has the code in std that runs before your `main`, Go has a whole
// runtime with a garbage collector.

#include <errno.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// Defined by the compiled program (see `emit_glue` in src/backend/llvm.rs).
extern const int64_t mini_input_count;
extern const char *const mini_input_names[];
int64_t mini_entry(const int64_t *inputs);

// Called by the compiled program (see `@mini_div` in the LLVM prelude).
void mini_division_by_zero(void) {
    fprintf(stderr, "error: division by zero\n");
    exit(1);
}

int main(int argc, char **argv) {
    if (argc - 1 != mini_input_count) {
        fprintf(stderr, "usage: %s", argv[0]);
        for (int64_t i = 0; i < mini_input_count; i++) {
            fprintf(stderr, " %s", mini_input_names[i]);
        }
        fprintf(stderr, "\n");
        return 2;
    }

    int64_t *inputs = calloc(mini_input_count + 1, sizeof(int64_t));
    for (int64_t i = 0; i < mini_input_count; i++) {
        const char *text = argv[i + 1];
        char *end;
        errno = 0;
        long long value = strtoll(text, &end, 10);
        if (end == text || *end != '\0' || errno == ERANGE) {
            fprintf(stderr, "error: input `%s` must be a 64-bit integer, got `%s`\n",
                    mini_input_names[i], text);
            return 2;
        }
        inputs[i] = value;
    }

    printf("%" PRId64 "\n", mini_entry(inputs));
    free(inputs);
    return 0;
}
