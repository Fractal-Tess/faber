/**
 * API error for the Faber SDK
 */

import { FaberError } from './faber-error';

/**
 * Thrown when the API returns an error
 */
export class ApiError extends FaberError {
  constructor(
    message: string,
    public readonly status: number,
    public readonly statusText?: string,
    public readonly response?: unknown
  ) {
    super(message, 'API_ERROR');
    this.name = 'ApiError';
    Object.setPrototypeOf(this, ApiError.prototype);
  }
}
