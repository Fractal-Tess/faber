import '@testing-library/jest-dom';

// Mock fetch for testing
global.fetch = vi.fn();

// Create mock response helper
export const createMockResponse = (ok: boolean, status: number, data: any) => {
  return Promise.resolve({
    ok,
    status,
    statusText: ok ? 'OK' : 'Error',
    json: () => Promise.resolve(data),
    text: () => Promise.resolve(JSON.stringify(data)),
  } as Response);
};

// Create mock fetch implementation
export const mockFetch = (response: any) => {
  const mockedFetch = vi.mocked(fetch);
  mockedFetch.mockResolvedValue(createMockResponse(true, 200, response));
  return mockedFetch;
};