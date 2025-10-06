/**
 * Integration test setup
 *
 * Environment variables expected:
 * - FABER_BASE_URL: Base URL for the Faber API server (default: http://localhost:3000)
 * - FABER_API_KEY: API key for authentication (default: just-a-test-api-key)
 */

export const getTestConfig = () => {
  const baseUrl = process.env.FABER_BASE_URL || 'http://localhost:3000';
  const apiKey = process.env.FABER_API_KEY || 'just-a-test-api-key';

  return {
    baseUrl,
    apiKey,
  };
};
