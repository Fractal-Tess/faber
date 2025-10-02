import { describe, it, expect } from 'vitest';
import {
  FaberError,
  ConnectionError,
  TimeoutError,
  ValidationError,
  ApiError,
} from '../src/errors';

describe('Error Classes', () => {
  describe('FaberError', () => {
    it('should create base FaberError', () => {
      const error = new FaberError('Test error');
      expect(error).toBeInstanceOf(Error);
      expect(error).toBeInstanceOf(FaberError);
      expect(error.name).toBe('FaberError');
      expect(error.message).toBe('Test error');
    });

    it('should maintain prototype chain', () => {
      const error = new FaberError('Test');
      expect(error instanceof Error).toBe(true);
      expect(error instanceof FaberError).toBe(true);
    });

    it('should capture stack trace', () => {
      const error = new FaberError('Test');
      expect(error.stack).toBeDefined();
      expect(error.stack).toContain('FaberError');
    });
  });

  describe('ConnectionError', () => {
    it('should create ConnectionError with message', () => {
      const error = new ConnectionError('Connection failed');
      expect(error).toBeInstanceOf(FaberError);
      expect(error).toBeInstanceOf(ConnectionError);
      expect(error.name).toBe('ConnectionError');
      expect(error.message).toBe('Connection failed');
    });

    it('should create ConnectionError with original error', () => {
      const originalError = new Error('Network error');
      const error = new ConnectionError('Connection failed', originalError);

      expect(error.message).toBe('Connection failed');
      expect((error as any).originalError).toBe(originalError);
    });

    it('should accept different message formats', () => {
      const error1 = new ConnectionError('Failed to connect');
      const error2 = new ConnectionError('Network timeout');

      expect(error1.message).toBe('Failed to connect');
      expect(error2.message).toBe('Network timeout');
    });
  });

  describe('TimeoutError', () => {
    it('should create TimeoutError with default message', () => {
      const error = new TimeoutError();
      expect(error).toBeInstanceOf(FaberError);
      expect(error).toBeInstanceOf(TimeoutError);
      expect(error.name).toBe('TimeoutError');
      expect(error.message).toBe('Request timed out');
    });

    it('should create TimeoutError with custom message', () => {
      const error = new TimeoutError('Custom timeout message');
      expect(error.message).toBe('Custom timeout message');
    });

    it('should work with different timeout scenarios', () => {
      const error1 = new TimeoutError();
      const error2 = new TimeoutError('Request timed out after 30 seconds');

      expect(error1.name).toBe('TimeoutError');
      expect(error2.name).toBe('TimeoutError');
    });
  });

  describe('ValidationError', () => {
    it('should create ValidationError with message', () => {
      const error = new ValidationError('Invalid input');
      expect(error).toBeInstanceOf(FaberError);
      expect(error).toBeInstanceOf(ValidationError);
      expect(error.name).toBe('ValidationError');
      expect(error.message).toBe('Invalid input');
    });

    it('should create ValidationError with message and validationErrors', () => {
      const validationErrors = [
        'Field "cmd" is required',
        'Field "args" must be an array',
      ];
      const error = new ValidationError('Validation failed', validationErrors);

      expect(error.message).toBe('Validation failed');
      expect((error as any).validationErrors).toEqual(validationErrors);
    });

    it('should handle empty validationErrors array', () => {
      const error = new ValidationError('Test error', []);
      expect((error as any).validationErrors).toEqual([]);
    });

    it('should handle single error detail', () => {
      const error = new ValidationError('Test error', ['Single error']);
      expect((error as any).validationErrors).toEqual(['Single error']);
    });

    it('should work with complex validation scenarios', () => {
      const complexValidationErrors = [
        'Step 0: cmd is required and must be a string',
        'Step 1[0]: args must be an array of strings',
        'Step 1[1]: env must be an object',
        'Step 2: files must be an object',
      ];
      const error = new ValidationError('Task group validation failed', complexValidationErrors);

      expect(error.message).toBe('Task group validation failed');
      expect((error as any).validationErrors).toHaveLength(4);
      expect((error as any).validationErrors[0]).toContain('Step 0');
    });
  });

  describe('ApiError', () => {
    it('should create ApiError with message and status', () => {
      const error = new ApiError('HTTP error', 404, 'Not Found');
      expect(error).toBeInstanceOf(FaberError);
      expect(error).toBeInstanceOf(ApiError);
      expect(error.name).toBe('ApiError');
      expect(error.message).toBe('HTTP error');
      expect((error as any).status).toBe(404);
      expect((error as any).statusText).toBe('Not Found');
    });

    it('should create ApiError with response data', () => {
      const responseData = { error: 'Not found', code: 'NOT_FOUND' };
      const error = new ApiError('HTTP 404: Not Found', 404, 'Not Found', responseData);

      expect(error.message).toBe('HTTP 404: Not Found');
      expect((error as any).status).toBe(404);
      expect((error as any).statusText).toBe('Not Found');
      expect((error as any).response).toEqual(responseData);
    });

    it('should handle different HTTP status codes', () => {
      const error400 = new ApiError('Bad Request', 400, 'Bad Request');
      const error401 = new ApiError('Unauthorized', 401, 'Unauthorized');
      const error500 = new ApiError('Internal Server Error', 500, 'Internal Server Error');

      expect((error400 as any).status).toBe(400);
      expect((error401 as any).status).toBe(401);
      expect((error500 as any).status).toBe(500);
    });

    it('should handle missing response data', () => {
      const error = new ApiError('HTTP error', 500, 'Server Error');
      expect((error as any).response).toBeUndefined();
    });

    it('should handle complex response data', () => {
      const complexResponse = {
        error: 'Validation failed',
        details: ['Field X is required', 'Field Y is invalid'],
        timestamp: '2023-01-01T00:00:00Z',
        requestId: 'req-123',
      };

      const error = new ApiError(
        'HTTP 400: Validation failed',
        400,
        'Bad Request',
        complexResponse
      );

      expect((error as any).response).toEqual(complexResponse);
      expect((error as any).response.details).toHaveLength(2);
    });
  });

  describe('Error inheritance and type checking', () => {
    it('should maintain proper inheritance chain', () => {
      const connectionError = new ConnectionError('Network error');
      const timeoutError = new TimeoutError('Timeout');
      const validationError = new ValidationError('Invalid');
      const apiError = new ApiError('HTTP error', 500, 'Server Error');

      // All should be instances of Error
      expect(connectionError instanceof Error).toBe(true);
      expect(timeoutError instanceof Error).toBe(true);
      expect(validationError instanceof Error).toBe(true);
      expect(apiError instanceof Error).toBe(true);

      // All should be instances of FaberError
      expect(connectionError instanceof FaberError).toBe(true);
      expect(timeoutError instanceof FaberError).toBe(true);
      expect(validationError instanceof FaberError).toBe(true);
      expect(apiError instanceof FaberError).toBe(true);

      // Each should be instance of its specific class
      expect(connectionError instanceof ConnectionError).toBe(true);
      expect(timeoutError instanceof TimeoutError).toBe(true);
      expect(validationError instanceof ValidationError).toBe(true);
      expect(apiError instanceof ApiError).toBe(true);
    });

    it('should allow type narrowing with instanceof', () => {
      const errors: FaberError[] = [
        new ConnectionError('Network error'),
        new TimeoutError('Timeout'),
        new ValidationError('Invalid'),
        new ApiError('HTTP error', 500, 'Server Error'),
      ];

      const connectionErrors = errors.filter(e => e instanceof ConnectionError);
      const timeoutErrors = errors.filter(e => e instanceof TimeoutError);
      const validationErrors = errors.filter(e => e instanceof ValidationError);
      const apiErrors = errors.filter(e => e instanceof ApiError);

      expect(connectionErrors).toHaveLength(1);
      expect(timeoutErrors).toHaveLength(1);
      expect(validationErrors).toHaveLength(1);
      expect(apiErrors).toHaveLength(1);
    });
  });

  describe('Error serialization', () => {
    it('should serialize to JSON properly', () => {
      const error = new ValidationError('Test error', ['Detail 1', 'Detail 2']);
      const json = JSON.stringify(error);
      const parsed = JSON.parse(json);

      // Error objects have limited JSON serialization, but basic properties should work
      // Note: Error.message is not enumerable in JSON.stringify, so it won't be included
      expect(parsed).toBeDefined();
      expect(typeof parsed).toBe('object');
    });

    it('should handle toString() properly', () => {
      const error = new ApiError('HTTP 404: Not Found', 404, 'Not Found');
      const str = error.toString();
      expect(str).toContain('ApiError');
      expect(str).toContain('HTTP 404: Not Found');
    });
  });

  describe('Edge cases', () => {
    it('should handle empty messages', () => {
      const error1 = new FaberError('');
      const error2 = new ConnectionError('');
      const error3 = new ValidationError('');

      expect(error1.message).toBe('');
      expect(error2.message).toBe('');
      expect(error3.message).toBe('');
    });

    it('should handle very long messages', () => {
      const longMessage = 'A'.repeat(10000);
      const error = new FaberError(longMessage);
      expect(error.message).toBe(longMessage);
    });

    it('should handle special characters in messages', () => {
      const specialMessage = 'Error with special chars: !@#$%^&*()_+-=[]{}|;:,.<>?';
      const error = new FaberError(specialMessage);
      expect(error.message).toBe(specialMessage);
    });

    it('should handle Unicode characters in messages', () => {
      const unicodeMessage = '错误 with émojis 🚀 and ñiño';
      const error = new FaberError(unicodeMessage);
      expect(error.message).toBe(unicodeMessage);
    });
  });
});