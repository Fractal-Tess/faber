/**
 * Example: C Program Compilation and Testing
 *
 * This example demonstrates how to use the Faber SDK to compile and test C programs
 * both with and without the TaskWithTest functionality.
 */

import { FaberClient, TaskBuilder, TaskWithTest, TestContext, TestResult } from '../src';

// Initialize the client
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: 'just-a-test-api-key'
});

async function basicCompilationExample() {
  console.log('=== Basic C Compilation Example ===\n');

  // Example 1: Simple Hello World program (no tests)
  console.log('1. Simple Hello World compilation (no tests):');
  const helloWorldResult = await client.executeGroup([
    {
      cmd: 'gcc',
      args: ['hello.c', '-o', 'hello'],
      files: {
        'hello.c': `
          #include <stdio.h>

          int main() {
            printf("Hello, World!\\n");
            printf("This is a test program.\\n");
            return 0;
          }
        `
      }
    },
    {
      cmd: './hello'
    }
  ]);

  console.log('Compilation result:', helloWorldResult[0]?.exit_code === 0 ? 'Success' : 'Failed');
  console.log('Program output:');
  console.log(helloWorldResult[1]?.stdout);
  console.log('');

  // Example 2: Program with mathematical functions (no tests)
  console.log('2. Mathematical program (no tests):');
  const mathResult = await client.executeGroup([
    {
      cmd: 'gcc',
      args: ['math.c', '-o', 'math', '-lm'],
      files: {
        'math.c': `
          #include <stdio.h>
          #include <math.h>

          double factorial(int n) {
            if (n <= 1) return 1;
            return n * factorial(n - 1);
          }

          int main() {
            printf("Factorial of 5: %.0f\\n", factorial(5));
            printf("Square root of 16: %.2f\\n", sqrt(16));
            printf("Power of 2^8: %.0f\\n", pow(2, 8));
            return 0;
          }
        `
      }
    },
    {
      cmd: './math'
    }
  ]);

  console.log('Compilation result:', mathResult[0]?.exit_code === 0 ? 'Success' : 'Failed');
  console.log('Program output:');
  console.log(mathResult[1]?.stdout);
  console.log('');
}

async function compilationWithTestsExample() {
  console.log('=== C Compilation with TaskWithTest ===\n');

  // Example 1: Hello World with comprehensive testing
  console.log('1. Hello World with test validation:');
  const helloWorldWithTests: TaskWithTest[] = [
    {
      cmd: 'gcc',
      args: ['hello.c', '-o', 'hello', '-Wall'],
      files: {
        'hello.c': `
          #include <stdio.h>

          int main() {
            printf("Hello from Faber!\\n");
            printf("Compilation test successful.\\n");
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
              : 'Compilation successful with no warnings',
          details: {
            exit_code: context.exit_code,
            has_warnings: hasWarnings,
            has_errors: hasErrors,
            compilation_time_ms: context.stats.execution_time_ms,
            memory_peak_bytes: context.stats.memory_peak_bytes
          }
        };
      }
    },
    {
      cmd: './hello',
      test: (context: TestContext): TestResult => {
        const expectedLines = ['Hello from Faber!', 'Compilation test successful.'];
        const actualLines = context.stdout.trim().split('\n');
        const allLinesPresent = expectedLines.every(line => context.stdout.includes(line));

        return {
          passed: context.exit_code === 0 && allLinesPresent,
          message: allLinesPresent
            ? 'Program output validation passed'
            : `Output mismatch. Expected lines: [${expectedLines.join(', ')}]`,
          details: {
            expected_lines: expectedLines,
            actual_lines: actualLines,
            exit_code: context.exit_code,
            execution_time_ms: context.stats.execution_time_ms,
            output_length: context.stdout.length
          }
        };
      }
    }
  ];

  const helloTestResult = await client.executeGroupWithTests(helloWorldWithTests);
  console.log(`All tests passed: ${helloTestResult.allTestsPassed}`);

  helloTestResult.stepResults.forEach((step, index) => {
    const stepName = index === 0 ? 'Compilation' : 'Execution';
    console.log(`  ${stepName}: ${step.passed ? '✅ PASS' : '❌ FAIL'} - ${step.testResult?.message || 'No test'}`);
  });
  console.log('');

  // Example 2: Calculator program with validation
  console.log('2. Calculator program with comprehensive tests:');
  const calculatorTasks: TaskWithTest[] = [
    {
      cmd: 'gcc',
      args: ['calculator.c', '-o', 'calculator', '-Wall', '-Wextra'],
      files: {
        'calculator.c': `
          #include <stdio.h>
          #include <stdlib.h>

          int add(int a, int b) { return a + b; }
          int subtract(int a, int b) { return a - b; }
          int multiply(int a, int b) { return a * b; }
          int divide(int a, int b) {
            if (b == 0) return 0;
            return a / b;
          }

          int main() {
            printf("10 + 5 = %d\\n", add(10, 5));
            printf("20 - 8 = %d\\n", subtract(20, 8));
            printf("6 * 7 = %d\\n", multiply(6, 7));
            printf("15 / 3 = %d\\n", divide(15, 3));
            printf("10 / 0 = %d (division by zero)\\n", divide(10, 0));
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
            ? `Compilation failed: ${context.stderr.substring(0, 100)}...`
            : hasWarnings
              ? `Compiled with warnings`
              : 'Compilation successful',
          details: {
            exit_code: context.exit_code,
            has_warnings: hasWarnings,
            has_errors: hasErrors,
            stderr_preview: context.stderr.substring(0, 200),
            compilation_time_ms: context.stats.execution_time_ms
          }
        };
      }
    },
    {
      cmd: './calculator',
      test: (context: TestContext): TestResult => {
        const expectedResults = [
          '10 + 5 = 15',
          '20 - 8 = 12',
          '6 * 7 = 42',
          '15 / 3 = 5',
          '10 / 0 = 0 (division by zero)'
        ];

        const outputLines = context.stdout.trim().split('\n');
        const allCorrect = expectedResults.every(expected =>
          outputLines.some(actual => actual.includes(expected.split(' = ')[1]))
        );

        return {
          passed: context.exit_code === 0 && allCorrect,
          message: allCorrect
            ? 'All calculations are correct'
            : 'Some calculations are incorrect',
          details: {
            expected_count: expectedResults.length,
            actual_lines: outputLines,
            exit_code: context.exit_code,
            execution_time_ms: context.stats.execution_time_ms,
            calculations_verified: allCorrect
          }
        };
      }
    }
  ];

  const calcTestResult = await client.executeGroupWithTests(calculatorTasks);
  console.log(`All tests passed: ${calcTestResult.allTestsPassed}`);

  calcTestResult.stepResults.forEach((step, index) => {
    const stepName = index === 0 ? 'Compilation' : 'Execution';
    console.log(`  ${stepName}: ${step.passed ? '✅ PASS' : '❌ FAIL'} - ${step.testResult?.message || 'No test'}`);
    if (step.testResult?.details) {
      console.log(`    Details: ${JSON.stringify(step.testResult.details, null, 6)}`);
    }
  });
  console.log('');

  // Example 3: Error handling test
  console.log('3. Compilation error handling test:');
  const errorTestTasks: TaskWithTest[] = [
    {
      cmd: 'gcc',
      args: ['broken.c', '-o', 'broken'],
      files: {
        'broken.c': `
          #include <stdio.h>

          int main() {
            printf("Missing semicolon here")
            return 1;
          }
        `
      },
      test: (context: TestContext): TestResult => {
        const hasCompileError = context.exit_code !== 0 && context.stderr.includes('error');

        return {
          passed: hasCompileError,
          message: hasCompileError
            ? 'Compilation error correctly detected'
            : 'Expected compilation error but compilation succeeded',
          details: {
            exit_code: context.exit_code,
            has_error: hasCompileError,
            stderr_preview: context.stderr.substring(0, 200),
            error_detected: hasCompileError
          }
        };
      }
    }
  ];

  const errorTestResult = await client.executeGroupWithTests(errorTestTasks);
  console.log(`Error handling test: ${errorTestResult.allTestsPassed ? '✅ PASS' : '❌ FAIL'}`);
  console.log(`Message: ${errorTestResult.stepResults[0]?.testResult?.message}`);
  console.log('');
}

async function taskBuilderExample() {
  console.log('=== TaskBuilder with C Compilation ===\n');

  // Example: Using TaskBuilder with tests
  console.log('Building and testing a sorting program using TaskBuilder:');

  const sortingProgramBuilder = new TaskBuilder()
    .singleWithTest({
      cmd: 'gcc',
      args: ['sort.c', '-o', 'sort', '-Wall'],
      files: {
        'sort.c': `
          #include <stdio.h>
          #include <stdlib.h>

          void bubbleSort(int arr[], int n) {
            for (int i = 0; i < n-1; i++) {
              for (int j = 0; j < n-i-1; j++) {
                if (arr[j] > arr[j+1]) {
                  int temp = arr[j];
                  arr[j] = arr[j+1];
                  arr[j+1] = temp;
                }
              }
            }
          }

          int main() {
            int arr[] = {64, 34, 25, 12, 22, 11, 90};
            int n = sizeof(arr)/sizeof(arr[0]);

            printf("Original array: ");
            for (int i = 0; i < n; i++) {
              printf("%d ", arr[i]);
            }

            bubbleSort(arr, n);

            printf("\\nSorted array: ");
            for (int i = 0; i < n; i++) {
              printf("%d ", arr[i]);
            }
            printf("\\n");

            return 0;
          }
        `
      }
    }, (context: TestContext): TestResult => {
      return {
        passed: context.exit_code === 0,
        message: context.exit_code === 0 ? 'Sorting program compiled successfully' : 'Compilation failed',
        details: {
          exit_code: context.exit_code,
          compilation_time_ms: context.stats.execution_time_ms
        }
      };
    })
    .singleWithTest({
      cmd: './sort'
    }, (context: TestContext): TestResult => {
      const hasOriginalArray = context.stdout.includes('64, 34, 25, 12, 22, 11, 90');
      const hasSortedArray = context.stdout.includes('11, 12, 22, 25, 34, 64, 90');

      return {
        passed: context.exit_code === 0 && hasOriginalArray && hasSortedArray,
        message: (hasOriginalArray && hasSortedArray)
          ? 'Sorting program works correctly'
          : 'Sorting program output is incorrect',
        details: {
          has_original: hasOriginalArray,
          has_sorted: hasSortedArray,
          exit_code: context.exit_code,
          execution_time_ms: context.stats.execution_time_ms,
          output_contains_both_arrays: hasOriginalArray && hasSortedArray
        }
      };
    });

  const builderResult = await client.executeGroupWithTests(sortingProgramBuilder);
  console.log(`TaskBuilder tests: ${builderResult.allTestsPassed ? '✅ ALL PASS' : '❌ SOME FAIL'}`);

  builderResult.stepResults.forEach((step, index) => {
    const stepName = index === 0 ? 'Compilation' : 'Execution';
    console.log(`  ${stepName}: ${step.passed ? '✅ PASS' : '❌ FAIL'} - ${step.testResult?.message || 'No test'}`);
  });
  console.log('');
}

// Run all examples
async function runAllExamples() {
  try {
    await basicCompilationExample();
    await compilationWithTestsExample();
    await taskBuilderExample();

    console.log('🎉 All C compilation examples completed successfully!');
  } catch (error) {
    console.error('❌ Error running examples:', error);
    process.exit(1);
  }
}

// Run if this file is executed directly
if (require.main === module) {
  runAllExamples();
}

export { basicCompilationExample, compilationWithTestsExample, taskBuilderExample, runAllExamples };