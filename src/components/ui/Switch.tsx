import { motion } from "motion/react";
import { useReducedMotion } from "@/hooks/useReducedMotion";

interface SwitchProps {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
  /** Tailwind className applied to the track. */
  className?: string;
  /** Visible label rendered inside (rare — usually used as a toggle pill). */
  label?: string;
  /** Accessible label for screen readers. */
  ariaLabel?: string;
}

/**
 * Apple-style iOS switch.
 * - Track width: 44px, height: 26px (iOS HIG)
 * - Thumb diameter: 22px with 2px padding (Apple spring-animates the thumb)
 * - Active colour: hsl(--primary)
 * - Pressed: thumb scales 0.92 via Motion's whileTap
 * - Reduced motion: snaps to position (still respects colour transition)
 */
export function Switch({ checked, onChange, disabled, className = "", label, ariaLabel }: SwitchProps) {
  const reduced = useReducedMotion();

  const SPRING = reduced
    ? { duration: 0.18 }
    : { type: "spring" as const, stiffness: 700, damping: 30, mass: 0.8 };

  return (
    <motion.button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={() => !disabled && onChange(!checked)}
      whileTap={disabled || reduced ? undefined : { scale: 0.95 }}
      transition={SPRING}
      className={`relative inline-flex h-[26px] w-[44px] flex-shrink-0 items-center rounded-full p-[2px] transition-colors duration-200 ease-out ${
        checked ? "bg-[hsl(var(--primary))]" : "bg-slate-300/80 dark:bg-white/15"
      } ${disabled ? "cursor-not-allowed opacity-50" : "cursor-pointer"} ${className}`}
    >
      <motion.span
        layout
        transition={SPRING}
        className="block h-[22px] w-[22px] rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.18),0_2px_6px_rgba(0,0,0,0.12)]"
        style={{ x: checked ? 18 : 0 }}
      />
      {label && (
        <span
          className={`pointer-events-none absolute inset-0 flex items-center text-[10px] font-medium ${
            checked ? "justify-start pl-1.5 text-white/90" : "justify-end pr-1.5 text-slate-500"
          }`}
        >
          {label}
        </span>
      )}
    </motion.button>
  );
}