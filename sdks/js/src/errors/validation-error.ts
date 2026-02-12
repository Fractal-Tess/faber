/**
 * Validation error for the Faber SDK
 */

import { FaberError } from './faber-error';

/**
 * Thrown when validation fails
 */
export class ValidationError extends FaberError {
  constructor(message: string, public readonly validationErrors?: string) {
    super(message, 'VALIDATION_ERROR');
    this.name = 'ValidationError';
    Object.setPrototypeOf(this, ValidationError.prototype);
  }
}

