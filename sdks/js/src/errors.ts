/**
 * Custom error classes for the Faber SDK
 */

export class FaberError extends Error {
  constructor(message: string, public code?: string, public details?: any) {
    super(message);
    this.name = 'FaberError';
  }
}

export class ConnectionError extends FaberError {
  constructor(message: string, public originalError?: Error) {
    super(message, 'CONNECTION_ERROR');
    this.name = 'ConnectionError';
  }
}

export class TimeoutError extends FaberError {
  constructor(message: string = 'Request timed out') {
    super(message, 'TIMEOUT_ERROR');
    this.name = 'TimeoutError';
  }
}

export class ValidationError extends FaberError {
  constructor(message: string, public validationErrors?: string[]) {
    super(message, 'VALIDATION_ERROR');
    this.name = 'ValidationError';
  }
}

export class ExecutionError extends FaberError {
  constructor(message: string, public exitCode?: number, public stderr?: string) {
    super(message, 'EXECUTION_ERROR');
    this.name = 'ExecutionError';
  }
}

export class ApiError extends FaberError {
  constructor(
    message: string,
    public status: number,
    public statusText?: string,
    public response?: any
  ) {
    super(message, 'API_ERROR');
    this.name = 'ApiError';
  }
}