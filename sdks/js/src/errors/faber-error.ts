/**
 * Base error class for the Faber SDK
 */

/**
 * Base error class for all Faber SDK errors
 */
export class FaberError extends Error {
  constructor(
    message: string,
    public readonly code?: string,
    public readonly details?: unknown
  ) {
    super(message);
    this.name = 'FaberError';
    Object.setPrototypeOf(this, FaberError.prototype);
  }
}
