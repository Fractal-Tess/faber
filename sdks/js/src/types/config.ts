/**
 * Configuration types for the Faber SDK
 */

/**
 * Configuration for the FaberClient
 * @property baseUrl - The base URL of the Faber server
 * @property apiKey - The API key for the Faber server
 * @property fetch - The fetch function to use for the FaberClient (optional)
 */
export interface FaberConfig {
  baseUrl: string;
  apiKey: string;
  fetch?: typeof fetch;
}

/**
 * Response from the FaberClient health check
 * @property status - The status of the Faber server
 */
export interface HealthResponse {
  status: string;
}

