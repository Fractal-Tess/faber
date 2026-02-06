import { z } from 'zod';

export const faberConfigSchema = z.object({
  baseUrl: z.url({ message: 'baseUrl must be a valid URL' }),
  apiKey: z.string({ message: 'apiKey is required' }),
  fetch: z.function().optional(),
});

export type FaberConfig = z.infer<typeof faberConfigSchema> & {
  fetch?: typeof fetch;
};
