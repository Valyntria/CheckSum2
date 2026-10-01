import { useEffect, useState } from 'react';
import { ALGORITHMS, Algorithm } from '../lib/types';

export function readAlgorithms(key: string): Algorithm[] {
  try {
    const raw = localStorage.getItem(key) ?? localStorage.getItem('hash-algorithms');
    if (!raw) return ['sha256'];
    const value: unknown = JSON.parse(raw);
    if (Array.isArray(value)) return ALGORITHMS.filter((a) => value.includes(a));
    if (typeof value === 'object' && value)
      return ALGORITHMS.filter((a) => (value as Record<string, unknown>)[a] === true);
  } catch {
    /* A corrupt or unavailable preference must not prevent startup. */
  }
  return ['sha256'];
}
export function useAlgorithms(key: string) {
  const [algorithms, setAlgorithms] = useState<Algorithm[]>(() => readAlgorithms(key));
  useEffect(() => {
    try {
      localStorage.setItem(key, JSON.stringify(algorithms));
    } catch {
      /* Storage may be unavailable. */
    }
  }, [key, algorithms]);
  return [algorithms, setAlgorithms] as const;
}
