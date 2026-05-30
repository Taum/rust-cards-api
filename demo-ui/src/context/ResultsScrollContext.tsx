import { createContext, useContext } from 'react';

export const ResultsScrollContext = createContext<HTMLDivElement | null>(null);

export function useResultsScrollRoot(): HTMLDivElement | null {
  return useContext(ResultsScrollContext);
}
