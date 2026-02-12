/**
 * Timeout error for the Faber SDK
 */

import { FaberError } from './faber-error';

/**
 * Thrown when a request times out
 */
export class TimeoutError extends FaberError {
  constructor(message: string = 'Request timed out') {
    super(message, 'TIMEOUT_ERROR');
    this.name = 'TimeoutError';
    Object.setPrototypeOf(this, TimeoutError.prototype);
  }
}

