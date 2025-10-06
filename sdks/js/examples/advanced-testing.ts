/**
 * Example: Advanced Testing Scenarios
 *
 * This example demonstrates advanced testing scenarios including performance testing,
 * error condition testing, and parallel execution with tests.
 */

import {
  FaberClient,
  TaskBuilder,
  TaskWithTest,
  TestContext,
  TestResult,
} from '../src';

// Initialize the client
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: 'just-a-test-api-key',
});

async function performanceTestingExample() {
  console.log('=== Performance Testing Example ===\n');

  // Example 1: Performance test for a CPU-intensive task
  console.log('1. CPU-intensive program with performance validation:');
  const performanceTasks: TaskWithTest[] = [
    {
      cmd: 'gcc',
      args: ['cpu_test.c', '-o', 'cpu_test', '-O2'],
      files: {
        'cpu_test.c': `
          #include <stdio.h>
          #include <time.h>
          #include <unistd.h>

          long long fibonacci(int n) {
            if (n <= 1) return n;
            return fibonacci(n-1) + fibonacci(n-2);
          }

          int main() {
            clock_t start = clock();

            // Calculate fibonacci(35) - should take some time
            long long result = fibonacci(35);

            clock_t end = clock();
            double cpu_time_used = ((double) (end - start)) / CLOCKS_PER_SEC;

            printf("Fibonacci(35) = %lld\\n", result);
            printf("CPU time used: %.4f seconds\\n", cpu_time_used);

            return 0;
          }
        `,
      },
      test: (context: TestContext): TestResult => {
        return {
          passed: context.exit_code === 0,
          message:
            context.exit_code === 0
              ? 'CPU test compiled successfully'
              : 'Compilation failed',
          details: { exit_code: context.exit_code },
        };
      },
    },
    {
      cmd: './cpu_test',
      test: (context: TestContext): TestResult => {
        const hasFibonacciResult = context.stdout.includes('Fibonacci(35) = ');
        const hasTiming = context.stdout.includes('CPU time used:');
        const executionTimeMs = context.stats.execution_time_ms;

        return {
          passed:
            context.exit_code === 0 &&
            hasFibonacciResult &&
            hasTiming &&
            executionTimeMs < 10000,
          message:
            executionTimeMs < 10000
              ? 'Performance test passed - execution within limits'
              : `Performance test failed - took too long (${executionTimeMs}ms)`,
          details: {
            has_fibonacci_result: hasFibonacciResult,
            has_timing_info: hasTiming,
            execution_time_ms: executionTimeMs,
            max_allowed_time_ms: 10000,
            memory_peak_bytes: context.stats.memory_peak_bytes,
            cpu_usage_percent: context.stats.cpu_usage_percent,
          },
        };
      },
    },
  ];

  const perfResult = await client.executeGroupWithTests(performanceTasks);
  console.log(
    `Performance test: ${perfResult.allTestsPassed ? '✅ PASS' : '❌ FAIL'}`
  );

  const execStep = perfResult.stepResults[1];
  if (execStep.testResult?.details) {
    console.log(`  Execution time: ${execStep.testResult.details.execution_time_ms}ms`);
    console.log(`  Memory peak: ${execStep.testResult.details.memory_peak_bytes} bytes`);
    console.log(`  CPU usage: ${execStep.testResult.details.cpu_usage_percent}%`);
  }
  console.log('');
}

async function errorConditionTestingExample() {
  console.log('=== Error Condition Testing Example ===\n');

  // Example 1: Testing various error conditions
  console.log('1. Testing compilation error scenarios:');
  const errorScenarios: TaskWithTest[] = [
    {
      cmd: 'gcc',
      args: ['syntax_error.c', '-o', 'syntax_error'],
      files: {
        'syntax_error.c': `
          #include <stdio.h>

          int main() {
            printf("Missing semicolon here")
            return 1;
          }
        `,
      },
      test: (context: TestContext): TestResult => {
        const hasSyntaxError =
          context.exit_code !== 0 &&
          (context.stderr.includes('syntax error') ||
            context.stderr.includes('expected') ||
            context.stderr.includes(';'));

        return {
          passed: hasSyntaxError,
          message: hasSyntaxError
            ? 'Syntax error correctly detected by compiler'
            : 'Expected syntax error but compilation succeeded',
          details: {
            exit_code: context.exit_code,
            has_syntax_error: hasSyntaxError,
            stderr_contains_semicolon_error: context.stderr.includes(';'),
            stderr_preview: context.stderr.substring(0, 150),
          },
        };
      },
    },
    {
      cmd: 'gcc',
      args: ['undefined_symbol.c', '-o', 'undefined_symbol'],
      files: {
        'undefined_symbol.c': `
          #include <stdio.h>

          int main() {
            undefined_function(); // This function doesn't exist
            return 0;
          }
        `,
      },
      test: (context: TestContext): TestResult => {
        const hasUndefinedError =
          context.exit_code !== 0 &&
          (context.stderr.includes('undefined') ||
            context.stderr.includes('declared') ||
            context.stderr.includes('not found'));

        return {
          passed: hasUndefinedError,
          message: hasUndefinedError
            ? 'Undefined symbol error correctly detected'
            : 'Expected undefined symbol error but compilation succeeded',
          details: {
            exit_code: context.exit_code,
            has_undefined_error: hasUndefinedError,
            stderr_contains_undefined: context.stderr.includes('undefined'),
            compilation_failed_as_expected: hasUndefinedError,
          },
        };
      },
    },
  ];

  const errorResult = await client.executeGroupWithTests(errorScenarios);
  console.log(
    `Error condition tests: ${
      errorResult.allTestsPassed ? '✅ ALL PASS' : '❌ SOME FAIL'
    }`
  );

  errorResult.stepResults.forEach((step, index) => {
    const testName = index === 0 ? 'Syntax Error' : 'Undefined Symbol';
    console.log(
      `  ${testName}: ${step.passed ? '✅ PASS' : '❌ FAIL'} - ${step.testResult?.message || 'No test'}`
    );
  });
  console.log('');
}

async function parallelTestingExample() {
  console.log('=== Parallel Execution with Tests ===\n');

  // Example 1: Parallel compilation and testing of multiple programs
  console.log('1. Parallel compilation of multiple C programs:');

  const programs = [
    {
      name: 'Calculator',
      code: `
        #include <stdio.h>
        int main() { printf("2+3=%d\\n", 2+3); return 0; }
      `,
    },
    {
      name: 'Greeter',
      code: `
        #include <stdio.h>
        int main() { printf("Hello, Parallel World!\\n"); return 0; }
      `,
    },
    {
      name: 'Counter',
      code: `
        #include <stdio.h>
        int main() {
          for(int i=1; i<=3; i++) printf("Count: %d\\n", i);
          return 0;
        }
      `,
    },
  ];

  const parallelTasks: TaskWithTest[] = programs
    .map((program) => {
      if (!program || !program.name || !program.code) {
        throw new Error(
          `Invalid program definition: ${JSON.stringify(program)}`
        );
      }

      return {
        cmd: 'gcc',
        args: [
          `${program.name.toLowerCase()}.c`,
          '-o',
          program.name.toLowerCase(),
        ],
        files: {
          [`${program.name.toLowerCase()}.c`]: program.code,
        },
        test: (context: TestContext): TestResult => ({
          passed: context.exit_code === 0,
          message:
            context.exit_code === 0
              ? `${program.name} compiled successfully`
              : `${program.name} compilation failed`,
          details: {
            program_name: program.name,
            exit_code: context.exit_code,
            compilation_time_ms: context.stats.execution_time_ms,
          },
        }),
      };
    })
    .filter((task) => task && task.cmd);

  const parallelCompileResult = await client.executeGroupWithTests(
    parallelTasks
  );
  console.log(
    `Parallel compilation: ${
      parallelCompileResult.allTestsPassed ? '✅ ALL PASS' : '❌ SOME FAIL'
    }`
  );

  parallelCompileResult.stepResults.forEach((step) => {
    if (Array.isArray(step.testResult)) {
      // Parallel step - show all test results
      step.testResult.forEach((test) => {
        console.log(
          `  ${test.details?.program_name}: ${
            test.passed ? '✅ PASS' : '❌ FAIL'
          } - ${test.message}`
        );
      });
    } else if (step.testResult) {
      // Single step
      console.log(
        `  ${step.testResult.details?.program_name}: ${
          step.testResult.passed ? '✅ PASS' : '❌ FAIL'
        } - ${step.testResult.message}`
      );
    }
  });
  console.log('');

  // Example 2: Parallel execution with validation
  console.log('2. Parallel execution of compiled programs:');
  const executionTasks: TaskWithTest[] = programs
    .map((program) => {
      if (!program || !program.name) {
        throw new Error(
          `Invalid program definition for execution: ${JSON.stringify(program)}`
        );
      }

      return {
        cmd: `./${program.name.toLowerCase()}`,
        test: (context: TestContext): TestResult => {
          let expectedOutput = '';
          let validationLogic = (output: string) => true;

          switch (program.name) {
            case 'Calculator':
              expectedOutput = '2+3=5';
              validationLogic = (output) => output.includes('2+3=5');
              break;
            case 'Greeter':
              expectedOutput = 'Hello, Parallel World!';
              validationLogic = (output) =>
                output.includes('Hello, Parallel World!');
              break;
            case 'Counter':
              expectedOutput = 'Count: 1';
              validationLogic = (output) =>
                output.includes('Count: 1') && output.includes('Count: 3');
              break;
          }

          const isValid = validationLogic(context.stdout);

          return {
            passed: context.exit_code === 0 && isValid,
            message: isValid
              ? `${program.name} execution validation passed`
              : `${program.name} execution validation failed`,
            details: {
              program_name: program.name,
              exit_code: context.exit_code,
              expected_output: expectedOutput,
              actual_output: context.stdout.trim(),
              validation_passed: isValid,
              execution_time_ms: context.stats.execution_time_ms,
            },
          };
        },
      };
    })
    .filter((task) => task && task.cmd);

  const parallelExecResult = await client.executeGroupWithTests(executionTasks);
  console.log(
    `Parallel execution: ${
      parallelExecResult.allTestsPassed ? '✅ ALL PASS' : '❌ SOME FAIL'
    }`
  );

  parallelExecResult.stepResults.forEach((step) => {
    if (Array.isArray(step.testResult)) {
      // Parallel step - show all test results
      step.testResult.forEach((test) => {
        console.log(
          `  ${test.details?.program_name}: ${
            test.passed ? '✅ PASS' : '❌ FAIL'
          } - ${test.message}`
        );
      });
    } else if (step.testResult) {
      // Single step
      console.log(
        `  ${step.testResult.details?.program_name}: ${
          step.testResult.passed ? '✅ PASS' : '❌ FAIL'
        } - ${step.testResult.message}`
      );
    }
  });
  console.log('');
}

async function complexWorkflowExample() {
  console.log('=== Complex Workflow with TaskBuilder ===\n');

  // Example: Build, test, and validate a complete project
  console.log('Building and testing a multi-file project:');

  const complexProject = new TaskBuilder()
    // Compile main program
    .singleWithTest(
      {
        cmd: 'gcc',
        args: ['-c', 'main.c', '-o', 'main.o'],
        files: {
          'main.c': `
          #include <stdio.h>
          #include "utils.h"

          int main() {
            printf("Starting complex workflow...\\n");
            int result = add_numbers(5, 3);
            printf("Add result: %d\\n", result);
            return 0;
          }
        `,
          'utils.h': `
          #ifndef UTILS_H
          #define UTILS_H

          int add_numbers(int a, int b);

          #endif
        `,
        },
      },
      (context: TestContext): TestResult => ({
        passed: context.exit_code === 0,
        message:
          context.exit_code === 0
            ? 'Main object file created'
            : 'Main compilation failed',
        details: { exit_code: context.exit_code },
      })
    )

    // Compile utilities
    .singleWithTest(
      {
        cmd: 'gcc',
        args: ['-c', 'utils.c', '-o', 'utils.o'],
        files: {
          'utils.c': `
          #include "utils.h"

          int add_numbers(int a, int b) {
            return a + b;
          }
        `,
        },
      },
      (context: TestContext): TestResult => ({
        passed: context.exit_code === 0,
        message:
          context.exit_code === 0
            ? 'Utils object file created'
            : 'Utils compilation failed',
        details: { exit_code: context.exit_code },
      })
    )

    // Link the program
    .singleWithTest(
      {
        cmd: 'gcc',
        args: ['main.o', 'utils.o', '-o', 'complex_project'],
      },
      (context: TestContext): TestResult => ({
        passed: context.exit_code === 0,
        message:
          context.exit_code === 0
            ? 'Project linked successfully'
            : 'Linking failed',
        details: { exit_code: context.exit_code },
      })
    )

    // Run and test the final program
    .singleWithTest(
      {
        cmd: './complex_project',
      },
      (context: TestContext): TestResult => {
        const hasStartMessage = context.stdout.includes(
          'Starting complex workflow'
        );
        const hasAddResult = context.stdout.includes('Add result: 8');

        return {
          passed: context.exit_code === 0 && hasStartMessage && hasAddResult,
          message:
            hasStartMessage && hasAddResult
              ? 'Complex workflow completed successfully'
              : 'Complex workflow validation failed',
          details: {
            has_start_message: hasStartMessage,
            has_correct_result: hasAddResult,
            exit_code: context.exit_code,
            total_execution_time_ms: context.stats.execution_time_ms,
            output_preview: context.stdout.trim(),
          },
        };
      }
    );

  const complexResult = await client.executeGroupWithTests(complexProject);
  console.log(
    `Complex workflow: ${
      complexResult.allTestsPassed ? '✅ ALL PASS' : '❌ SOME FAIL'
    }`
  );

  complexResult.stepResults.forEach((step, index) => {
    const stepNames = ['Main Compile', 'Utils Compile', 'Link', 'Execution'];
    console.log(
      `  ${stepNames[index]}: ${step.passed ? '✅ PASS' : '❌ FAIL'} - ${
        step.testResult?.message || 'No test'
      }`
    );
  });

  if (!complexResult.allTestsPassed) {
    console.log('\nFailed steps:');
    complexResult.failedSteps.forEach((step, index) => {
      console.log(`  Step ${index + 1}: ${step.testResult?.message || 'Step failed'}`);
    });
  }
  console.log('');
}

// Run all examples
async function runAllExamples() {
  try {
    await performanceTestingExample();
    await errorConditionTestingExample();
    await parallelTestingExample();
    await complexWorkflowExample();

    console.log('🎉 All advanced testing examples completed successfully!');
  } catch (error) {
    console.error('❌ Error running examples:', error);
    process.exit(1);
  }
}

// Run if this file is executed directly
if (require.main === module) {
  runAllExamples();
}

export {
  performanceTestingExample,
  errorConditionTestingExample,
  parallelTestingExample,
  complexWorkflowExample,
  runAllExamples,
};
