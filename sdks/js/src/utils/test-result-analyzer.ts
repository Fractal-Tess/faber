import type {
  ExecutionWithTestsResult,
  StepWithTestsResult,
  TaskTestResult,
} from '../types/tests';

export type FailedStepInfo = {
  stage: number;
  type: 'single' | 'parallel';
  taskIndex?: number;
  cmd: string;
  failedTests: TaskTestResult[];
  allTests: TaskTestResult[];
};

export type StageSummary =
  | {
      stage: number;
      type: 'single';
      passed: boolean;
      totalTests: number;
      passedTests: number;
      failedTests: number;
    }
  | {
      stage: number;
      type: 'parallel';
      passed: boolean;
      totalTasks: number;
      passedTasks: number;
      failedTasks: number;
    };

export type ReportOptions = {
  showPassed?: boolean;
  includeOutput?: boolean;
};

export class TestResultAnalyzer {
  constructor(private result: ExecutionWithTestsResult) {}

  get allPassed(): boolean {
    return this.result.allTestsPassed;
  }

  get totalSteps(): number {
    return this.result.stepResults.length;
  }

  get passedSteps(): number {
    return this.result.stepResults.filter(s => s.passed).length;
  }

  get failedSteps(): number {
    return this.result.stepResults.filter(s => !s.passed).length;
  }

  getFailedSteps(): FailedStepInfo[] {
    const failed: FailedStepInfo[] = [];

    for (const step of this.result.stepResults) {
      if (!step.passed) {
        if (step.parallel) {
          step.results.forEach((taskResult, taskIndex) => {
            if (!taskResult.testsPassed) {
              failed.push({
                stage: step.stepIndex,
                type: 'parallel',
                taskIndex,
                cmd: taskResult.task.cmd,
                failedTests: taskResult.testResults.filter(t => !t.passed),
                allTests: taskResult.testResults,
              });
            }
          });
        } else {
          failed.push({
            stage: step.stepIndex,
            type: 'single',
            cmd: step.task.cmd,
            failedTests: step.testResults.filter(t => !t.passed),
            allTests: step.testResults,
          });
        }
      }
    }

    return failed;
  }

  getFirstFailure(): FailedStepInfo | null {
    const failed = this.getFailedSteps();
    return failed.length > 0 ? failed[0] : null;
  }

  getStageFailure(stageIndex: number): FailedStepInfo | null {
    const step = this.result.stepResults[stageIndex];
    if (!step || step.passed) return null;

    if (step.parallel) {
      const failedTask = step.results.find(r => !r.testsPassed);
      if (failedTask) {
        return {
          stage: stageIndex,
          type: 'parallel',
          taskIndex: step.results.indexOf(failedTask),
          cmd: failedTask.task.cmd,
          failedTests: failedTask.testResults.filter(t => !t.passed),
          allTests: failedTask.testResults,
        };
      }
    } else {
      return {
        stage: stageIndex,
        type: 'single',
        cmd: step.task.cmd,
        failedTests: step.testResults.filter(t => !t.passed),
        allTests: step.testResults,
      };
    }
    return null;
  }

  getStageSummary(): StageSummary[] {
    return this.result.stepResults.map((step, index) => {
      if (step.parallel) {
        return {
          stage: index,
          type: 'parallel',
          passed: step.passed,
          totalTasks: step.results.length,
          passedTasks: step.results.filter(r => r.testsPassed).length,
          failedTasks: step.results.filter(r => !r.testsPassed).length,
        };
      } else {
        return {
          stage: index,
          type: 'single',
          passed: step.passed,
          totalTests: step.testResults.length,
          passedTests: step.testResults.filter(t => t.passed).length,
          failedTests: step.testResults.filter(t => !t.passed).length,
        };
      }
    });
  }

  formatReport(options: ReportOptions = {}): string {
    const { showPassed = false, includeOutput = false } = options;
    const lines: string[] = [];

    lines.push('═'.repeat(60));
    lines.push(
      `TEST RESULTS: ${this.allPassed ? '✓ ALL PASSED' : '✗ FAILURES FOUND'}`
    );
    lines.push('═'.repeat(60));
    lines.push(
      `Total Stages: ${this.totalSteps} | Passed: ${this.passedSteps} | Failed: ${this.failedSteps}`
    );
    lines.push('');

    lines.push('STAGE SUMMARY:');
    lines.push('-'.repeat(60));
    this.getStageSummary().forEach(stage => {
      const status = stage.passed ? '✓' : '✗';
      if (stage.type === 'parallel') {
        lines.push(
          `  ${status} Stage ${stage.stage}: Parallel (${stage.passedTasks}/${stage.totalTasks} tasks passed)`
        );
      } else {
        lines.push(
          `  ${status} Stage ${stage.stage}: Single (${stage.passedTests}/${stage.totalTests} tests passed)`
        );
      }
    });
    lines.push('');

    if (!this.allPassed) {
      lines.push('FAILURE DETAILS:');
      lines.push('-'.repeat(60));

      this.getFailedSteps().forEach((failure, index) => {
        lines.push(`\n${index + 1}. Stage ${failure.stage} (${failure.type})`);
        lines.push(`   Command: ${failure.cmd}`);
        if (failure.taskIndex !== undefined) {
          lines.push(`   Task Index: ${failure.taskIndex}`);
        }
        lines.push('');
        lines.push('   Failed Tests:');
        failure.failedTests.forEach(test => {
          lines.push(`     ✗ ${test.name}`);
          lines.push(`       ${test.message}`);
          if (test.expected !== undefined) {
            lines.push(`       Expected: ${test.expected}`);
            lines.push(`       Actual: ${test.actual}`);
          }
        });

        if (includeOutput && failure.allTests.length > 0) {
          lines.push('');
          lines.push('   All Tests:');
          failure.allTests.forEach(test => {
            const icon = test.passed ? '✓' : '✗';
            lines.push(`     ${icon} ${test.name}: ${test.message}`);
          });
        }
      });
    }

    if (showPassed && !this.allPassed) {
      lines.push('');
      lines.push('PASSED STAGES:');
      lines.push('-'.repeat(60));
      this.result.stepResults
        .filter(s => s.passed)
        .forEach(step => {
          lines.push(`  ✓ Stage ${step.stepIndex}`);
        });
    }

    lines.push('');
    lines.push('═'.repeat(60));

    return lines.join('\n');
  }

  assertAllPassed(message?: string): void {
    if (!this.allPassed) {
      const report = this.formatReport({ includeOutput: true });
      throw new Error(message || `Tests failed:\n${report}`);
    }
  }

  get raw(): ExecutionWithTestsResult {
    return this.result;
  }
}
