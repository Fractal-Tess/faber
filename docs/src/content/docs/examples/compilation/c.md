---
title: C Compilation
description: Compile and run C programs with Faber
---

# C/C++ Compilation

Faber can compile and run C/C++ programs within isolated containers.

## Prerequisites

Use a custom Docker image with GCC installed:

```dockerfile
FROM vgfractal/faber AS faber
FROM debian:latest

RUN apt-get update && apt-get install -y \
    gcc \
    g++ \
    make \
    libc-dev

WORKDIR /opt
COPY --from=faber /opt/faber /opt

EXPOSE 3000/tcp
ENTRYPOINT ["./faber"]
```

Build and run:

```bash
docker build -t faber-gcc .
docker run --privileged --cgroupns=host -p 3000:3000 faber-gcc
```

## Basic C Program

Compile and run a simple C program:

```typescript
const builder = new TaskBuilder()
  // Step 1: Compile
  .single({
    cmd: '/usr/bin/gcc',
    args: ['hello.c', '-o', 'hello'],
    files: {
      'hello.c': `
#include <stdio.h>

int main() {
    printf("Hello from C!\\n");
    return 0;
}
      `,
    },
  })
  // Step 2: Run
  .single({ cmd: './hello' });

const results = await client.executeGroup(builder);

console.log('Compile exit code:', results[0].exitCode); // 0
console.log('Program output:', results[1].stdout); // "Hello from C!\n"
```

## With Compiler Warnings

Enable all warnings:

```typescript
const builder = new TaskBuilder()
  .single({
    cmd: '/usr/bin/gcc',
    args: ['-Wall', '-Wextra', '-o', 'program', 'main.c'],
    files: {
      'main.c': `
#include <stdio.h>

int main() {
    int x = 42;
    printf("The answer is %d\\n", x);
    return 0;
}
      `,
    },
  })
  .single({ cmd: './program' });

const results = await client.executeGroup(builder);
console.log(results[1].stdout); // "The answer is 42\n"
```

## Optimization Levels

Compile with optimizations:

```typescript
const builder = new TaskBuilder()
  .single({
    cmd: '/usr/bin/gcc',
    args: ['-O2', '-o', 'optimized', 'program.c'],
    files: {
      'program.c': '/* program code */',
    },
  });
```

Optimization levels:
- `-O0` - No optimization (default)
- `-O1` - Basic optimization
- `-O2` - Recommended optimization
- `-O3` - Aggressive optimization
- `-Os` - Optimize for size

## Multiple Source Files

Compile a project with multiple files:

```typescript
const builder = new TaskBuilder()
  .single({
    cmd: '/usr/bin/gcc',
    args: ['-o', 'program', 'main.c', 'utils.c', 'lib.c'],
    files: {
      'main.c': `
#include <stdio.h>
#include "utils.h"

int main() {
    print_message();
    return 0;
}
      `,
      'utils.c': `
#include <stdio.h>
#include "utils.h"

void print_message() {
    printf("Hello from utils!\\n");
}
      `,
      'utils.h': `
#ifndef UTILS_H
#define UTILS_H
void print_message();
#endif
      `,
      'lib.c': `
// Library implementation
      `,
    },
  })
  .single({ cmd: './program' });
```

## With Tests

Validate compilation and output:

```typescript
const builder = new TaskBuilder()
  .singleWithTests(
    {
      cmd: '/usr/bin/gcc',
      args: ['-Wall', '-o', 'calc', 'calc.c'],
      files: {
        'calc.c': `
#include <stdio.h>

int add(int a, int b) {
    return a + b;
}

int main() {
    printf("2 + 3 = %d\\n", add(2, 3));
    return 0;
}
        `,
      },
    },
    [
      {
        name: 'compiles without errors',
        assertion: 'equals',
        field: 'exitCode',
        expected: 0,
      },
      {
        name: 'no warnings',
        assertion: 'equals',
        field: 'stderr',
        expected: '',
      },
    ]
  )
  .singleWithTests(
    { cmd: './calc' },
    [
      {
        name: 'outputs correct result',
        assertion: 'contains',
        field: 'stdout',
        expected: '2 + 3 = 5',
      },
    ]
  );

const result = await client.executeWithTests(builder);
console.log('All tests passed:', result.allTestsPassed);
```

## Handling Compilation Errors

Check for compilation failures:

```typescript
const result = await client.executeSingle({
  cmd: '/usr/bin/gcc',
  args: ['-o', 'broken', 'broken.c'],
  files: {
    'broken.c': `
#include <stdio.h>

int main() {
    printf("Missing semicolon")
    return 0;
}
    `,
  },
});

if (result.exitCode !== 0) {
  console.error('Compilation failed!');
  console.error('Errors:', result.stderr);
  // Output: "error: expected ';' before 'return'"
}
```

## C++ Compilation

Compile C++ programs:

```typescript
const builder = new TaskBuilder()
  .single({
    cmd: '/usr/bin/g++',
    args: ['-std=c++17', '-o', 'cpp_program', 'main.cpp'],
    files: {
      'main.cpp': `
#include <iostream>
#include <vector>
#include <string>

int main() {
    std::vector<std::string> messages = {"Hello", "from", "C++"};
    
    for (const auto& msg : messages) {
        std::cout << msg << " ";
    }
    std::cout << std::endl;
    
    return 0;
}
      `,
    },
  })
  .single({ cmd: './cpp_program' });

const results = await client.executeGroup(builder);
console.log(results[1].stdout); // "Hello from C++ "
```

## Makefiles

Use Make for complex builds:

```typescript
const builder = new TaskBuilder()
  .single({
    cmd: '/usr/bin/make',
    files: {
      'Makefile': `
CC=gcc
CFLAGS=-Wall -Wextra -O2

program: main.o utils.o
	$(CC) $(CFLAGS) -o program main.o utils.o

main.o: main.c utils.h
	$(CC) $(CFLAGS) -c main.c

utils.o: utils.c utils.h
	$(CC) $(CFLAGS) -c utils.c

clean:
	rm -f *.o program
      `,
      'main.c': '/* main implementation */',
      'utils.c': '/* utils implementation */',
      'utils.h': '/* utils header */',
    },
  })
  .single({ cmd: './program' });
```

## Benchmarking

Measure compilation and execution time:

```typescript
const builder = new TaskBuilder()
  .single({
    cmd: '/usr/bin/gcc',
    args: ['-O2', '-o', 'fib', 'fib.c'],
    files: {
      'fib.c': `
#include <stdio.h>

long fib(int n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

int main() {
    printf("fib(40) = %ld\\n", fib(40));
    return 0;
}
      `,
    },
  })
  .single({ cmd: './fib' });

const results = await client.executeGroup(builder);

console.log('Compile time:', results[0].stats?.execution_time_ms, 'ms');
console.log('Run time:', results[1].stats?.execution_time_ms, 'ms');
console.log('Output:', results[1].stdout);
```

## Next Steps

- [Basic Patterns](/examples/patterns/basic/) - General usage patterns
- [Python Scripts](/examples/python/basic/) - Run Python programs
- [TaskBuilder Guide](/sdk/taskbuilder/overview/) - Build complex workflows
