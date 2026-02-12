/**
 * Execution error for the Faber SDK
 */

import { FaberError } from './faber-error';

/**
 * Thrown when task execution fails
 */
export class ExecutionError extends FaberError {
  constructor(
    message: string,
    public readonly exitCode?: number,
    public readonly stderr?: string
  ) {
    super(message, 'EXECUTION_ERROR');
    this.name = 'ExecutionError';
    Object.setPrototypeOf(this, ExecutionError.prototype);
  }
}

