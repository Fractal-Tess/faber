/**
 * Example: Basic Task Execution
 *
 * Demonstrates how to use the Faber SDK to execute tasks
 * both sequentially and in parallel.
 */

import { FaberClient, TaskGroup } from '../src';

// Create a client instance
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: 'just-a-test-api-key',
});

// Create a task group
const taskGroup = new TaskGroup(client);

// Add a single task followed by parallel tasks
taskGroup
  .single({
    cmd: 'echo',
    args: ['Hello, world!'],
  })
  .parallel([
    {
      cmd: 'echo',
      args: ['Task 1'],
    },
    {
      cmd: 'echo',
      args: ['Task 2'],
    },
  ]);

// Execute and measure performance
const startTime = performance.now();
const results = await taskGroup.execute();
const endTime = performance.now();

console.log(`Execution time: ${(endTime - startTime).toFixed(2)}ms`);
console.log('Results:', JSON.stringify(results, null, 2));
