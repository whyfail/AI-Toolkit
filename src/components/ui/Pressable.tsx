import { motion, type HTMLMotionProps } from "motion/react";
import { forwardRef } from "react";
import { useReducedMotion } from "@/hooks/useReducedMotion";

export type PressableVariant = "primary" | "secondary" | "danger" | "icon" | "ghost";

interface PressableProps extends Omit<HTMLMotionProps<"button">, "ref"> {
  variant?: PressableVariant;
  /** Disable press scale while keeping click behaviour. */
  noScale?: boolean;
}

/**
 * Apple-style pressable button.
 * - While pressed: scale 0.97 (Apple's standard press feedback).
 * - While pressed (icon variant): scale 0.92 — smaller hit, larger travel.
 * - Default to critically-damped spring so press can be reversed at any time.
 * - Reduced-motion: scale replaced by opacity flash.
 */
export const Pressable = forwardRef<HTMLButtonElement, PressableProps>(
  function Pressable({ variant = "secondary", noScale, className, children, disabled, ...rest }, ref) {
    const reduced = useReducedMotion();
    const pressScale = variant === "icon" ? 0.92 : 0.97;

    const whileTap = noScale || disabled || reduced ? undefined : { scale: pressScale };
    const whileHover =
      noScale || disabled || reduced
        ? undefined
        : { filter: variant === "primary" || variant === "danger" ? "brightness(1.05)" : "brightness(1.02)" };

    return (
      <motion.button
        ref={ref}
        whileTap={whileTap}
        whileHover={whileHover}
        transition={{ type: "spring", bounce: 0, duration: 0.18 }}
        className={className}
        disabled={disabled}
        {...rest}
      >
        {children}
      </motion.button>
    );
  },
);