/**
 * Connection error for the Faber SDK
 */

import { FaberError } from './faber-error';

/**
 * Thrown when there is a connection error with the Faber server
 */
export class ConnectionError extends FaberError {
  constructor(message: string, public readonly originalError?: Error) {
    super(message, 'CONNECTION_ERROR');
    this.name = 'ConnectionError';
    Object.setPrototypeOf(this, ConnectionError.prototype);
  }
}

