import { useReducedMotion as useMotionReducedMotion } from "motion/react";

/**
 * Wrapper around motion's `useReducedMotion` that:
 *  - normalises the value to a boolean
 *  - provides a sensible default (`false` during SSR)
 *
 * Use this in every component that runs an animated effect so users
 * with `prefers-reduced-motion: reduce` automatically get the gentler
 * non-vestibular equivalent.
 */
export function useReducedMotion(): boolean {
  return Boolean(useMotionReducedMotion());
}