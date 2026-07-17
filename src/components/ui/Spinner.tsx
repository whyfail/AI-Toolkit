import { motion } from "motion/react";
import { Loader2 } from "lucide-react";
import { useReducedMotion } from "@/hooks/useReducedMotion";

interface SpinnerProps {
  size?: number;
  /** Tailwind className for colour. Default `text-current`. */
  className?: string;
}

/**
 * Apple-style looping spinner.
 * - Uses motion.rotate so reduced-motion can drop the spin entirely
 *   (still renders, just no rotation animation).
 */
export function Spinner({ size = 16, className = "text-current" }: SpinnerProps) {
  const reduced = useReducedMotion();
  return (
    <motion.span
      className={`inline-flex ${className}`}
      animate={reduced ? undefined : { rotate: 360 }}
      transition={reduced ? undefined : { duration: 1, ease: "linear", repeat: Infinity }}
    >
      <Loader2 size={size} />
    </motion.span>
  );
}