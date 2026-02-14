import { describe, it, expect, beforeAll } from 'vitest';
import { FaberClient } from '../src/client';
import { TaskBuilder } from '../src/builders/task-builder';
import { getTestConfig } from './setup.integration';

describe('C Program Compilation Integration Tests', () => {
  let client: FaberClient;
  const config = getTestConfig();

  beforeAll(() => {
    client = new FaberClient({
      baseUrl: config.baseUrl,
      apiKey: config.apiKey,
    });
  });

  describe('Basic compilation', () => {
    it('should compile a simple hello world program', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['hello.c', '-o', 'hello'],
          files: {
            'hello.c': `
              #include <stdio.h>
              int main() {
                printf("Hello, World!\\n");
                return 0;
              }
            `,
          },
        },
        {
          cmd: './hello',
        },
      ]);

      expect(result).toBeDefined();
      expect(result.length).toBe(2);

      const compileResult = result[0];
      if (!Array.isArray(compileResult)) {
        expect(compileResult.exitCode).toBe(0);
      }

      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Hello, World!');
        expect(runResult.exitCode).toBe(0);
      }
    });

    it('should compile with optimization flags', async () => {
      const result = await client.executeSingle({
        cmd: 'gcc',
        args: ['program.c', '-o', 'program', '-O2', '-Wall'],
        files: {
          'program.c': `
            #include <stdio.h>
            int main() {
              int sum = 0;
              for (int i = 0; i < 100; i++) {
                sum += i;
              }
              printf("Sum: %d\\n", sum);
              return 0;
            }
          `,
        },
      });

      expect(result).toBeDefined();
      expect(result.exitCode).toBe(0);
    });

    it('should compile with debug symbols', async () => {
      const result = await client.executeSingle({
        cmd: 'gcc',
        args: ['debug.c', '-o', 'debug', '-g', '-Wall'],
        files: {
          'debug.c': `
            #include <stdio.h>
            int main() {
              int x = 42;
              printf("Debug value: %d\\n", x);
              return 0;
            }
          `,
        },
      });

      expect(result).toBeDefined();
      expect(result.exitCode).toBe(0);
    });
  });

  describe('Compilation errors', () => {
    it('should handle syntax errors gracefully', async () => {
      const result = await client.executeSingle({
        cmd: 'gcc',
        args: ['syntax_error.c', '-o', 'syntax_error'],
        files: {
          'syntax_error.c': `
            #include <stdio.h>
            int main() {
              printf("Missing semicolon")
              return 0;
            }
          `,
        },
      });

      expect(result).toBeDefined();
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toBeTruthy();
    });

    it('should handle missing include errors', async () => {
      const result = await client.executeSingle({
        cmd: 'gcc',
        args: ['missing_include.c', '-o', 'missing_include'],
        files: {
          'missing_include.c': `
            #include <nonexistent.h>
            int main() {
              return 0;
            }
          `,
        },
      });

      expect(result).toBeDefined();
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain('nonexistent.h');
    });

    it('should handle undefined reference errors', async () => {
      const result = await client.executeSingle({
        cmd: 'gcc',
        args: ['undefined_ref.c', '-o', 'undefined_ref'],
        files: {
          'undefined_ref.c': `
            int main() {
              extern void nonexistent_function();
              nonexistent_function();
              return 0;
            }
          `,
        },
      });

      expect(result).toBeDefined();
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toBeTruthy();
    });
  });

  describe('Multi-file compilation', () => {
    it('should compile and link multiple source files', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['main.c', 'utils.c', '-o', 'program'],
          files: {
            'main.c': `
              #include <stdio.h>
              extern int add(int a, int b);
              
              int main() {
                int result = add(5, 7);
                printf("Result: %d\\n", result);
                return 0;
              }
            `,
            'utils.c': `
              int add(int a, int b) {
                return a + b;
              }
            `,
          },
        },
        {
          cmd: './program',
        },
      ]);

      expect(result).toBeDefined();
      expect(result.length).toBe(2);

      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Result: 12');
        expect(runResult.exitCode).toBe(0);
      }
    });

    it('should compile object files and link separately', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['-c', 'math.c', '-o', 'math.o'],
          files: {
            'math.c': `
              int multiply(int a, int b) {
                return a * b;
              }
            `,
          },
        },
        {
          cmd: 'gcc',
          args: ['-c', 'main.c', '-o', 'main.o'],
          files: {
            'main.c': `
              #include <stdio.h>
              extern int multiply(int a, int b);
              
              int main() {
                printf("Product: %d\\n", multiply(6, 7));
                return 0;
              }
            `,
          },
        },
        {
          cmd: 'gcc',
          args: ['main.o', 'math.o', '-o', 'program'],
        },
        {
          cmd: './program',
        },
      ]);

      expect(result).toBeDefined();
      expect(result.length).toBe(4);

      const runResult = result[3];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Product: 42');
        expect(runResult.exitCode).toBe(0);
      }
    });
  });

  describe('Header files', () => {
    it('should compile with custom header files', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['program.c', '-o', 'program'],
          files: {
            'program.c': `
              #include <stdio.h>
              #include "myheader.h"
              
              int main() {
                printf("Value: %d\\n", MY_CONSTANT);
                return 0;
              }
            `,
            'myheader.h': `
              #ifndef MYHEADER_H
              #define MYHEADER_H
              #define MY_CONSTANT 42
              #endif
            `,
          },
        },
        {
          cmd: './program',
        },
      ]);

      expect(result).toBeDefined();
      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Value: 42');
        expect(runResult.exitCode).toBe(0);
      }
    });

    it('should compile with function declarations in headers', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['main.c', 'functions.c', '-o', 'program'],
          files: {
            'functions.h': `
              #ifndef FUNCTIONS_H
              #define FUNCTIONS_H
              int square(int x);
              int cube(int x);
              #endif
            `,
            'functions.c': `
              #include "functions.h"
              
              int square(int x) {
                return x * x;
              }
              
              int cube(int x) {
                return x * x * x;
              }
            `,
            'main.c': `
              #include <stdio.h>
              #include "functions.h"
              
              int main() {
                printf("Square of 5: %d\\n", square(5));
                printf("Cube of 3: %d\\n", cube(3));
                return 0;
              }
            `,
          },
        },
        {
          cmd: './program',
        },
      ]);

      expect(result).toBeDefined();
      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Square of 5: 25');
        expect(runResult.stdout).toContain('Cube of 3: 27');
        expect(runResult.exitCode).toBe(0);
      }
    });
  });

  describe('Standard libraries', () => {
    it('should compile with math library', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['math_program.c', '-o', 'math_program', '-lm'],
          files: {
            'math_program.c': `
              #include <stdio.h>
              #include <math.h>
              
              int main() {
                double result = sqrt(16.0);
                printf("Square root of 16: %.1f\\n", result);
                return 0;
              }
            `,
          },
        },
        {
          cmd: './math_program',
        },
      ]);

      expect(result).toBeDefined();
      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Square root of 16: 4.0');
        expect(runResult.exitCode).toBe(0);
      }
    });

    it('should compile with string operations', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['string_program.c', '-o', 'string_program'],
          files: {
            'string_program.c': `
              #include <stdio.h>
              #include <string.h>
              
              int main() {
                char str1[20] = "Hello";
                char str2[20] = " World";
                strcat(str1, str2);
                printf("%s\\n", str1);
                printf("Length: %zu\\n", strlen(str1));
                return 0;
              }
            `,
          },
        },
        {
          cmd: './string_program',
        },
      ]);

      expect(result).toBeDefined();
      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Hello World');
        expect(runResult.stdout).toContain('Length: 11');
        expect(runResult.exitCode).toBe(0);
      }
    });
  });

  describe('Compiler warnings', () => {
    it('should show warnings with -Wall flag', async () => {
      const result = await client.executeSingle({
        cmd: 'gcc',
        args: ['warnings.c', '-o', 'warnings', '-Wall'],
        files: {
          'warnings.c': `
            #include <stdio.h>
            int main() {
              int unused_variable = 42;
              printf("Hello\\n");
              return 0;
            }
          `,
        },
      });

      expect(result).toBeDefined();
      expect(result.exitCode).toBe(0);
      if (result.stderr) {
        expect(result.stderr).toContain('unused');
      }
    });

    it('should treat warnings as errors with -Werror', async () => {
      const result = await client.executeSingle({
        cmd: 'gcc',
        args: ['warnings.c', '-o', 'warnings', '-Wall', '-Werror'],
        files: {
          'warnings.c': `
            #include <stdio.h>
            int main() {
              int unused_variable = 42;
              printf("Hello\\n");
              return 0;
            }
          `,
        },
      });

      expect(result).toBeDefined();
      expect(result.exitCode).not.toBe(0);
    });
  });

  describe('C standards', () => {
    it('should compile with C99 standard', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['c99_program.c', '-o', 'c99_program', '-std=c99'],
          files: {
            'c99_program.c': `
              #include <stdio.h>
              
              int main() {
                for (int i = 0; i < 5; i++) {
                  printf("%d ", i);
                }
                printf("\\n");
                return 0;
              }
            `,
          },
        },
        {
          cmd: './c99_program',
        },
      ]);

      expect(result).toBeDefined();
      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('0 1 2 3 4');
        expect(runResult.exitCode).toBe(0);
      }
    });

    it('should compile with C11 standard', async () => {
      const result = await client.executeSingle({
        cmd: 'gcc',
        args: ['c11_program.c', '-o', 'c11_program', '-std=c11'],
        files: {
          'c11_program.c': `
            #include <stdio.h>
            
            int main(void) {
              printf("C11 program\\n");
              return 0;
            }
          `,
        },
      });

      expect(result).toBeDefined();
      expect(result.exitCode).toBe(0);
    });
  });

  describe('TaskBuilder for compilation workflows', () => {
    it('should use TaskBuilder for compile and test workflow', async () => {
      const plan = new TaskBuilder()
        .single({
          cmd: 'gcc',
          args: ['calculator.c', '-o', 'calculator'],
          files: {
            'calculator.c': `
              #include <stdio.h>
              
              int add(int a, int b) { return a + b; }
              int subtract(int a, int b) { return a - b; }
              int multiply(int a, int b) { return a * b; }
              
              int main() {
                printf("Add: %d\\n", add(10, 5));
                printf("Subtract: %d\\n", subtract(10, 5));
                printf("Multiply: %d\\n", multiply(10, 5));
                return 0;
              }
            `,
          },
        })
        .parallel([
          {
            cmd: './calculator',
          },
        ]);

      const result = await client.executeGroup(plan);

      expect(result).toBeDefined();
      expect(result.length).toBe(2);

      const parallelResults = result[1];
      if (Array.isArray(parallelResults)) {
        expect(parallelResults[0].stdout).toContain('Add: 15');
        expect(parallelResults[0].stdout).toContain('Subtract: 5');
        expect(parallelResults[0].stdout).toContain('Multiply: 50');
      }
    });

    it('should compile multiple programs in parallel', async () => {
      const plan = new TaskBuilder().parallel([
        {
          cmd: 'gcc',
          args: ['prog1.c', '-o', 'prog1'],
          files: {
            'prog1.c': `
              #include <stdio.h>
              int main() {
                printf("Program 1\\n");
                return 0;
              }
            `,
          },
        },
        {
          cmd: 'gcc',
          args: ['prog2.c', '-o', 'prog2'],
          files: {
            'prog2.c': `
              #include <stdio.h>
              int main() {
                printf("Program 2\\n");
                return 0;
              }
            `,
          },
        },
        {
          cmd: 'gcc',
          args: ['prog3.c', '-o', 'prog3'],
          files: {
            'prog3.c': `
              #include <stdio.h>
              int main() {
                printf("Program 3\\n");
                return 0;
              }
            `,
          },
        },
      ]);

      const result = await client.executeGroup(plan);

      expect(result).toBeDefined();
      expect(result.length).toBe(1);

      const parallelResults = result[0];
      if (Array.isArray(parallelResults)) {
        expect(parallelResults.length).toBe(3);
        parallelResults.forEach((taskResult) => {
          expect(taskResult.exitCode).toBe(0);
        });
      }
    });
  });

  describe('Preprocessor', () => {
    it('should handle preprocessor directives', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['preprocessor.c', '-o', 'preprocessor', '-DDEBUG_MODE'],
          files: {
            'preprocessor.c': `
              #include <stdio.h>
              
              int main() {
                #ifdef DEBUG_MODE
                  printf("Debug mode enabled\\n");
                #else
                  printf("Debug mode disabled\\n");
                #endif
                return 0;
              }
            `,
          },
        },
        {
          cmd: './preprocessor',
        },
      ]);

      expect(result).toBeDefined();
      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Debug mode enabled');
        expect(runResult.exitCode).toBe(0);
      }
    });

    it('should use preprocessor macros', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'gcc',
          args: ['macros.c', '-o', 'macros', '-DMAX_SIZE=100'],
          files: {
            'macros.c': `
              #include <stdio.h>
              
              int main() {
                printf("Max size: %d\\n", MAX_SIZE);
                return 0;
              }
            `,
          },
        },
        {
          cmd: './macros',
        },
      ]);

      expect(result).toBeDefined();
      const runResult = result[1];
      if (!Array.isArray(runResult)) {
        expect(runResult.stdout).toContain('Max size: 100');
        expect(runResult.exitCode).toBe(0);
      }
    });
  });

  describe('Client-side tests with executeWithTests', () => {
    it('should validate compilation and execution with tests', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'gcc',
          args: ['calculator.c', '-o', 'calculator', '-Wall'],
          files: {
            'calculator.c': `
              #include <stdio.h>

              int add(int a, int b) { return a + b; }
              int multiply(int a, int b) { return a * b; }

              int main() {
                printf("5 + 3 = %d\\n", add(5, 3));
                printf("4 * 7 = %d\\n", multiply(4, 7));
                return 0;
              }
            `
          },
          tests: [
            {
              name: 'compilation succeeds',
              assertion: 'equals',
              field: 'exitCode',
              expected: 0,
            },
          ],
        },
        {
          cmd: './calculator',
          tests: [
            {
              name: 'output contains addition',
              assertion: 'contains',
              field: 'stdout',
              expected: '5 + 3 = 8',
            },
            {
              name: 'output contains multiplication',
              assertion: 'contains',
              field: 'stdout',
              expected: '4 * 7 = 28',
            },
          ],
        },
      ]);

      expect(result).toBeDefined();
      expect(result.results).toHaveLength(2);
      expect(result.stepResults).toHaveLength(2);
      expect(result.allTestsPassed).toBe(true);
    });

    it('should handle test failures correctly', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'gcc',
          args: ['broken.c', '-o', 'broken'],
          files: {
            'broken.c': `
              #include <stdio.h>
              int main() {
                printf("Hello")
                return 0;
              }
            `
          },
          tests: [
            {
              name: 'compilation succeeds',
              assertion: 'equals',
              field: 'exitCode',
              expected: 0,
            },
          ],
        },
      ]);

      expect(result).toBeDefined();
      expect(result.allTestsPassed).toBe(false);
      expect(result.failedCount).toBe(1);
    });

    it('should work with TaskBuilder and singleWithTests', async () => {
      const plan = new TaskBuilder().singleWithTests(
        { cmd: 'echo', args: ['Hello, World!'] },
        [
          {
            name: 'output check',
            assertion: 'contains',
            field: 'stdout',
            expected: 'Hello, World!',
          },
        ]
      );

      const result = await client.executeWithTests(plan);

      expect(result).toBeDefined();
      expect(result.allTestsPassed).toBe(true);
      expect(result.stepResults).toHaveLength(1);
    });

    it('should handle parallel tasks with tests', async () => {
      const result = await client.executeWithTests([
        [
          {
            cmd: 'echo',
            args: ['Task 1'],
            tests: [
              {
                name: 'task 1 check',
                assertion: 'contains',
                field: 'stdout',
                expected: 'Task 1',
              },
            ],
          },
          {
            cmd: 'echo',
            args: ['Task 2'],
            tests: [
              {
                name: 'task 2 check',
                assertion: 'contains',
                field: 'stdout',
                expected: 'Task 2',
              },
            ],
          },
        ],
      ]);

      expect(result).toBeDefined();
      expect(result.allTestsPassed).toBe(true);
      expect(result.stepResults).toHaveLength(1);
    });
  });
});