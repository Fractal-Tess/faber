/**
 * Array utility functions
 */

/**
 * Zips two arrays together into an array of tuples
 * @param a - First array
 * @param b - Second array
 * @returns Array of tuples [a[i], b[i]]
 */
export function zip<U, V>(a: U[], b: V[]): [U, V][] {
  return a.map((k, i) => [k, b[i]]);
}

