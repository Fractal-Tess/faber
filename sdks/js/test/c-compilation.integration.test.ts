import { describe, it, expect, beforeAll } from 'vitest';
import { FaberClient } from '../src/client';
import { TaskBuilder } from '../src/task-builder';
import { getTestConfig } from './setup.integration';
import type { TaskWithTest, TestContext, TestResult } from '../src/types';

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

      // Compilation step
      const compileResult = result[0];
      if (!Array.isArray(compileResult) && 'stdout' in compileResult) {
        expect(compileResult.exit_code).toBe(0);
      }

      // Execution step
      const runResult = result[1];
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Hello, World!');
        expect(runResult.exit_code).toBe(0);
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
      const stepResult = result[0];
      if (!Array.isArray(stepResult) && 'stdout' in stepResult) {
        expect(stepResult.exit_code).toBe(0);
      }
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
      const stepResult = result[0];
      if (!Array.isArray(stepResult) && 'stdout' in stepResult) {
        expect(stepResult.exit_code).toBe(0);
      }
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
      const stepResult = result[0];
      if (!Array.isArray(stepResult) && 'stdout' in stepResult) {
        // GCC returns non-zero exit code on compilation errors
        expect(stepResult.exit_code).not.toBe(0);
        expect(stepResult.stderr).toBeTruthy();
      }
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
      const stepResult = result[0];
      if (!Array.isArray(stepResult) && 'stdout' in stepResult) {
        expect(stepResult.exit_code).not.toBe(0);
        expect(stepResult.stderr).toContain('nonexistent.h');
      }
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
      const stepResult = result[0];
      if (!Array.isArray(stepResult) && 'stdout' in stepResult) {
        expect(stepResult.exit_code).not.toBe(0);
        expect(stepResult.stderr).toBeTruthy();
      }
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Result: 12');
        expect(runResult.exit_code).toBe(0);
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Product: 42');
        expect(runResult.exit_code).toBe(0);
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Value: 42');
        expect(runResult.exit_code).toBe(0);
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Square of 5: 25');
        expect(runResult.stdout).toContain('Cube of 3: 27');
        expect(runResult.exit_code).toBe(0);
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Square root of 16: 4.0');
        expect(runResult.exit_code).toBe(0);
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Hello World');
        expect(runResult.stdout).toContain('Length: 11');
        expect(runResult.exit_code).toBe(0);
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
      const stepResult = result[0];
      if (!Array.isArray(stepResult) && 'stdout' in stepResult) {
        // Compilation should succeed but may have warnings
        expect(stepResult.exit_code).toBe(0);
        // Check if stderr contains warning about unused variable
        if (stepResult.stderr) {
          expect(stepResult.stderr).toContain('unused');
        }
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
      const stepResult = result[0];
      if (!Array.isArray(stepResult) && 'stdout' in stepResult) {
        // Should fail because warnings are treated as errors
        expect(stepResult.exit_code).not.toBe(0);
      }
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
                // Variable declaration in for loop (C99 feature)
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('0 1 2 3 4');
        expect(runResult.exit_code).toBe(0);
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
      const stepResult = result[0];
      if (!Array.isArray(stepResult) && 'stdout' in stepResult) {
        expect(stepResult.exit_code).toBe(0);
      }
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
          {
            cmd: 'sh',
            args: ['-c', 'ls -la calculator'],
          },
        ]);

      const result = await client.executeGroup(plan);

      expect(result).toBeDefined();
      expect(result.length).toBe(2);

      // Check parallel execution results
      const parallelResults = result[1];
      if (Array.isArray(parallelResults)) {
        // First parallel task - run calculator
        if ('stdout' in parallelResults[0]) {
          expect(parallelResults[0].stdout).toContain('Add: 15');
          expect(parallelResults[0].stdout).toContain('Subtract: 5');
          expect(parallelResults[0].stdout).toContain('Multiply: 50');
        }

        // Second parallel task - list executable
        if ('stdout' in parallelResults[1]) {
          expect(parallelResults[1].stdout).toContain('calculator');
        }
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
          if ('stdout' in taskResult) {
            expect(taskResult.exit_code).toBe(0);
          }
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Debug mode enabled');
        expect(runResult.exit_code).toBe(0);
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
      if (!Array.isArray(runResult) && 'stdout' in runResult) {
        expect(runResult.stdout).toContain('Max size: 100');
        expect(runResult.exit_code).toBe(0);
      }
    });
  });

  describe('TaskWithTest functionality', () => {
    it('should validate compilation and execution with built-in tests', async () => {
      const tasksWithTests: TaskWithTest[] = [
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
          test: (context: TestContext): TestResult => {
            const hasWarnings = context.stderr.includes('warning');
            const hasErrors = context.stderr.includes('error');

            return {
              passed: context.exit_code === 0 && !hasErrors,
              message: hasErrors
                ? `Compilation failed: ${context.stderr}`
                : hasWarnings
                  ? `Compiled with warnings: ${context.stderr}`
                  : 'Compilation successful',
              details: {
                exit_code: context.exit_code,
                has_warnings: hasWarnings,
                has_errors: hasErrors,
                compilation_time: context.stats.execution_time_ms,
                memory_used: context.stats.memory_peak_bytes
              }
            };
          }
        },
        {
          cmd: './calculator',
          test: (context: TestContext): TestResult => {
            const expectedOutput = ['5 + 3 = 8', '4 * 7 = 28'];
            const outputLines = context.stdout.trim().split('\n');
            const allLinesPresent = expectedOutput.every(line =>
              context.stdout.includes(line)
            );

            return {
              passed: context.exit_code === 0 && allLinesPresent,
              message: allLinesPresent
                ? 'Program output is correct'
                : `Program output mismatch. Expected lines: ${expectedOutput.join(', ')}, Got: ${outputLines.join(', ')}`,
              details: {
                exit_code: context.exit_code,
                expected_lines: expectedOutput,
                actual_lines: outputLines,
                execution_time: context.stats.execution_time_ms,
                output_length: context.stdout.length
              }
            };
          }
        }
      ];

      const result = await client.executeGroupWithTests(tasksWithTests);

      expect(result).toBeDefined();
      expect(result.results).toHaveLength(2);
      expect(result.stepResults).toHaveLength(2);
      expect(result.allTestsPassed).toBe(true);
      expect(result.failedSteps).toHaveLength(0);

      // Check individual step results
      const compilationStep = result.stepResults[0];
      expect(compilationStep.passed).toBe(true);
      expect(compilationStep.testResult).toBeDefined();
      expect(compilationStep.testResult!.passed).toBe(true);
      expect(compilationStep.testResult!.message).toContain('Compilation successful');

      const executionStep = result.stepResults[1];
      expect(executionStep.passed).toBe(true);
      expect(executionStep.testResult).toBeDefined();
      expect(executionStep.testResult!.passed).toBe(true);
      expect(executionStep.testResult!.message).toContain('Program output is correct');
    });

    it('should handle test failures correctly', async () => {
      const tasksWithTests: TaskWithTest[] = [
        {
          cmd: 'gcc',
          args: ['broken.c', '-o', 'broken'],
          files: {
            'broken.c': `
              #include <stdio.h>
              int main() {
                printf("Hello")  // Missing semicolon
                return 0;
              }
            `
          },
          test: (context: TestContext): TestResult => {
            return {
              passed: context.exit_code === 0,
              message: context.exit_code === 0 ? 'Compilation succeeded' : 'Compilation failed',
              details: { exit_code: context.exit_code }
            };
          }
        }
      ];

      const result = await client.executeGroupWithTests(tasksWithTests);

      expect(result).toBeDefined();
      expect(result.allTestsPassed).toBe(false);
      expect(result.failedSteps).toHaveLength(1);

      const failedStep = result.failedSteps[0];
      expect(failedStep.passed).toBe(false);
      expect(failedStep.testResult).toBeDefined();
      expect(failedStep.testResult!.passed).toBe(false);
      expect(failedStep.testResult!.message).toContain('Compilation failed');
    });

    it('should work with TaskBuilder and test functions', async () => {
      const taskBuilder = new TaskBuilder()
        .singleWithTest({
          cmd: 'echo',
          args: ['Hello, World!']
        }, (context: TestContext): TestResult => {
          return {
            passed: context.stdout.includes('Hello, World!') && context.exit_code === 0,
            message: context.stdout.includes('Hello, World!')
              ? 'Echo command worked correctly'
              : 'Echo command failed',
            details: {
              output: context.stdout.trim(),
              exit_code: context.exit_code,
              execution_time: context.stats.execution_time_ms
            }
          };
        });

      const result = await client.executeGroupWithTests(taskBuilder);

      expect(result).toBeDefined();
      expect(result.allTestsPassed).toBe(true);
      expect(result.stepResults).toHaveLength(1);

      const stepResult = result.stepResults[0];
      expect(stepResult.passed).toBe(true);
      expect(stepResult.testResult).toBeDefined();
      expect(stepResult.testResult!.passed).toBe(true);
      expect(stepResult.testResult!.message).toContain('Echo command worked correctly');
    });

    it('should handle parallel tasks with tests', async () => {
      const parallelTasks: TaskWithTest[] = [
        {
          cmd: 'echo',
          args: ['Task 1'],
          test: (context: TestContext): TestResult => ({
            passed: context.stdout.includes('Task 1'),
            message: context.stdout.includes('Task 1') ? 'Task 1 passed' : 'Task 1 failed',
            details: { output: context.stdout.trim() }
          })
        },
        {
          cmd: 'echo',
          args: ['Task 2'],
          test: (context: TestContext): TestResult => ({
            passed: context.stdout.includes('Task 2'),
            message: context.stdout.includes('Task 2') ? 'Task 2 passed' : 'Task 2 failed',
            details: { output: context.stdout.trim() }
          })
        }
      ];

      // For parallel execution, we need to pass the array as a single step
      const result = await client.executeGroupWithTests([parallelTasks]);

      expect(result).toBeDefined();
      expect(result.allTestsPassed).toBe(true);
      expect(result.stepResults).toHaveLength(1);

      // This is a parallel step with two tasks
      const parallelStep = result.stepResults[0];
      expect(parallelStep.passed).toBe(true);
      expect(parallelStep.testResult).toBeDefined();
      expect(Array.isArray(parallelStep.testResult)).toBe(true);

      const parallelTestResults = parallelStep.testResult as TestResult[];
      expect(parallelTestResults).toHaveLength(2);
      expect(parallelTestResults[0].passed).toBe(true);
      expect(parallelTestResults[1].passed).toBe(true);
    });
  });
});
