# Faber JavaScript/TypeScript SDK

A JavaScript/TypeScript client SDK for the [Faber](https://github.com/Fractal-Tess/faber) secure task execution runtime.

## Installation

```bash
npm install @faber/runtime-sdk
```

## Example

```typescript
import { FaberClient, TaskBuilder } from '@faber/runtime-sdk';

// Create a client with API key authentication
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: process.env.FABER_API_KEY, // Or your API key string
  fetch: customFetch, // Optional: provide your own fetch implementation (for timeouts, custom headers, etc.)
});

// Use the TaskBuilder for complex workflows
const plan = new TaskBuilder()
  .single({
    cmd: 'gcc',
    args: ['greeter.c', '-o', 'greeter'],
    env: {
      CC: 'gcc',
      CFLAGS: '-O2',
    },
    files: {
      'greeter.c': `
        #include <stdio.h>
        #include <string.h>

        int main() {
          char name[100];
          char company[100];

          printf("Enter your name: ");
          if (fgets(name, sizeof(name), stdin)) {
            name[strcspn(name, "\\n")] = 0;

            printf("Enter your company: ");
            if (fgets(company, sizeof(company), stdin)) {
              company[strcspn(company, "\\n")] = 0;

              printf("Hello %s from %s!\\n", name, company);
              printf("Welcome to Faber!\\n");
            }
          }
          return 0;
        }
      `,
    },
  })
  .parallel([
    {
      cmd: './greeter',
      stdin: 'Alice\nTechCorp\n',
    },
    {
      cmd: './greeter',
      stdin: 'Bob\nStartupInc\n',
    },
  ]);

const executionResult = await client.executeGroup(plan);
```
