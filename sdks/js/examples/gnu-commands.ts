/**
 * Example: Basic GNU Commands
 *
 * This example demonstrates how to use the Faber SDK to execute basic GNU/Linux commands
 * both with and without the TaskWithTest functionality.
 */

import { FaberClient, TaskWithTest, TestContext, TestResult } from '../src';

// Initialize the client
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: 'just-a-test-api-key'
});

async function basicCommandsExample() {
  console.log('=== Basic GNU Commands Example ===\n');

  // Example 1: Simple command without tests
  console.log('1. Simple echo command (no tests):');
  const echoResult = await client.executeSingle({
    cmd: 'echo',
    args: ['Hello from Faber SDK!']
  });
  console.log('Output:', echoResult[0]?.stdout?.trim());
  console.log('Exit code:', echoResult[0]?.exit_code);
  console.log('');

  // Example 2: Directory listing without tests
  console.log('2. Directory listing (no tests):');
  const lsResult = await client.executeSingle({
    cmd: 'ls',
    args: ['-la', '/tmp']
  });
  console.log('Files in /tmp:');
  console.log(lsResult[0]?.stdout);
  console.log('');

  // Example 3: File operations without tests
  console.log('3. File operations (no tests):');
  const fileOps = await client.executeGroup([
    {
      cmd: 'sh',
      args: ['-c', 'echo "Hello World" > /tmp/faber-test.txt']
    },
    {
      cmd: 'cat',
      args: ['/tmp/faber-test.txt']
    },
    {
      cmd: 'rm',
      args: ['/tmp/faber-test.txt']
    }
  ]);

  console.log('File creation result:', fileOps[0]?.exit_code === 0 ? 'Success' : 'Failed');
  console.log('File content:', fileOps[1]?.stdout?.trim());
  console.log('File deletion result:', fileOps[2]?.exit_code === 0 ? 'Success' : 'Failed');
  console.log('');
}

async function commandsWithTestsExample() {
  console.log('=== GNU Commands with TaskWithTest ===\n');

  // Example 1: Echo command with validation
  console.log('1. Echo command with test validation:');
  const echoTask: TaskWithTest = {
    cmd: 'echo',
    args: ['Hello, World!'],
    test: (context: TestContext): TestResult => {
      const expectedOutput = 'Hello, World!';
      const actualOutput = context.stdout.trim();

      return {
        passed: actualOutput === expectedOutput && context.exit_code === 0,
        message: actualOutput === expectedOutput
          ? 'Echo command produced expected output'
          : `Expected "${expectedOutput}", got "${actualOutput}"`,
        details: {
          expected_output: expectedOutput,
          actual_output: actualOutput,
          exit_code: context.exit_code,
          execution_time_ms: context.stats.execution_time_ms
        }
      };
    }
  };

  const echoTestResult = await client.executeGroupWithTests([echoTask]);
  console.log(`Test passed: ${echoTestResult.allTestsPassed}`);
  console.log(`Message: ${echoTestResult.stepResults[0]?.testResult?.message}`);
  console.log('');

  // Example 2: Directory listing with validation
  console.log('2. Directory listing with test validation:');
  const lsTask: TaskWithTest = {
    cmd: 'ls',
    args: ['/etc'],
    test: (context: TestContext): TestResult => {
      const hasFiles = context.stdout.trim().length > 0;
      const hasCommonFiles = context.stdout.includes('passwd') || context.stdout.includes('hosts');

      return {
        passed: context.exit_code === 0 && hasFiles,
        message: context.exit_code === 0 && hasFiles
          ? 'Directory listing successful'
          : `Directory listing failed (exit code: ${context.exit_code})`,
        details: {
          exit_code: context.exit_code,
          has_files: hasFiles,
          has_common_files: hasCommonFiles,
          line_count: context.stdout.split('\n').length,
          execution_time_ms: context.stats.execution_time_ms
        }
      };
    }
  };

  const lsTestResult = await client.executeGroupWithTests([lsTask]);
  console.log(`Test passed: ${lsTestResult.allTestsPassed}`);
  console.log(`Message: ${lsTestResult.stepResults[0]?.testResult?.message}`);
  console.log(`Details: ${JSON.stringify(lsTestResult.stepResults[0]?.testResult?.details, null, 2)}`);
  console.log('');

  // Example 3: Multiple commands with comprehensive testing
  console.log('3. Multi-step file operations with tests:');
  const fileTasks: TaskWithTest[] = [
    {
      cmd: 'sh',
      args: ['-c', 'echo "Faber SDK Test File" > /tmp/faber-example.txt'],
      test: (context: TestContext): TestResult => ({
        passed: context.exit_code === 0,
        message: context.exit_code === 0 ? 'File created successfully' : 'File creation failed',
        details: { exit_code: context.exit_code }
      })
    },
    {
      cmd: 'wc',
      args: ['-w', '/tmp/faber-example.txt'],
      test: (context: TestContext): TestResult => {
        const wordCount = parseInt(context.stdout.trim());
        return {
          passed: context.exit_code === 0 && wordCount === 4,
          message: wordCount === 4
            ? 'Word count is correct (4 words)'
            : `Expected 4 words, got ${wordCount}`,
          details: {
            word_count: wordCount,
            expected: 4,
            raw_output: context.stdout.trim(),
            exit_code: context.exit_code
          }
        };
      }
    },
    {
      cmd: 'grep',
      args: ['-q', 'Faber', '/tmp/faber-example.txt'],
      test: (context: TestContext): TestResult => ({
        passed: context.exit_code === 0,
        message: context.exit_code === 0 ? 'Content validation passed' : 'Content validation failed',
        details: {
          found_content: context.exit_code === 0,
          exit_code: context.exit_code
        }
      })
    },
    {
      cmd: 'rm',
      args: ['/tmp/faber-example.txt'],
      test: (context: TestContext): TestResult => ({
        passed: context.exit_code === 0,
        message: context.exit_code === 0 ? 'File cleaned up successfully' : 'File cleanup failed',
        details: { exit_code: context.exit_code }
      })
    }
  ];

  const multiTestResult = await client.executeGroupWithTests(fileTasks);
  console.log(`All tests passed: ${multiTestResult.allTestsPassed}`);
  console.log(`Number of steps: ${multiTestResult.stepResults.length}`);

  multiTestResult.stepResults.forEach((step, index) => {
    console.log(`  Step ${index + 1}: ${step.passed ? '✅ PASS' : '❌ FAIL'} - ${step.testResult?.message || 'No test'}`);
  });
  console.log('');

  if (!multiTestResult.allTestsPassed) {
    console.log('Failed steps:');
    multiTestResult.failedSteps.forEach(step => {
      console.log(`  - ${step.testResult?.message || 'Step failed'}`);
    });
  }
}

// Run all examples
async function runAllExamples() {
  try {
    await basicCommandsExample();
    await commandsWithTestsExample();

    console.log('🎉 All examples completed successfully!');
  } catch (error) {
    console.error('❌ Error running examples:', error);
    process.exit(1);
  }
}

// Run if this file is executed directly
if (require.main === module) {
  runAllExamples();
}

export { basicCommandsExample, commandsWithTestsExample, runAllExamples };