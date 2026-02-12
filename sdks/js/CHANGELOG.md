# @faber/runtime-sdk

## 0.2.0

### Minor Changes

- Faber SDK v0.2.0 - Complete implementation with TaskBuilder, client-side testing, and comprehensive test suite

  ## Features

  - **TaskBuilder**: Fluent API for building task sequences with single(), parallel(), singleWithTests(), parallelWithTests()
  - **Enhanced FaberClient**: New methods health(), executeSingle(), executeGroup(), executeWithTests()
  - **Client-Side Testing Framework**: Define tests on tasks with equals, contains, matches, and custom assertions
  - **TestResultAnalyzer**: Detailed test result inspection with getFailedSteps(), formatReport(), assertAllPassed()
  - **Full TypeScript Support**: Complete type definitions for all SDK components

  ## Testing

  - 106 unit tests (all passing)
  - 30 integration tests across 4 test files
  - Test coverage for TaskBuilder, test-runner, test utilities, and TestResultAnalyzer

  ## Documentation

  - Comprehensive README with API reference, examples, and usage guides
  - 575 lines of documentation covering all SDK features
