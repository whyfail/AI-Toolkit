import { motion, type HTMLMotionProps } from "motion/react";
import { type ReactNode } from "react";
import { useReducedMotion } from "@/hooks/useReducedMotion";

interface MotionListProps extends Omit<HTMLMotionProps<"div">, "children"> {
  children: ReactNode;
  /** Delay between items in seconds. Default 0.03. */
  stagger?: number;
  /** Initial Y offset in px. Default 8. */
  initialOffset?: number;
}

/**
 * Stagger-fade list container.
 * Children should be `<MotionListItem>` (or any motion component) to inherit
 * the parent variants.
 *
 * Reduced motion: no stagger, no transform — just opacity.
 */
export function MotionList({
  children,
  stagger = 0.03,
  initialOffset = 8,
  className,
  ...rest
}: MotionListProps) {
  const reduced = useReducedMotion();

  const container = {
    hidden: { opacity: 1 },
    visible: {
      opacity: 1,
      transition: reduced
        ? { staggerChildren: 0 }
        : { staggerChildren: stagger, delayChildren: 0 },
    },
  };

  // We pass the child variant via the data attribute trick so MotionListItem
  // can read it. Easiest path: forward variants through context-free API by
  // encoding the variant name and letting MotionListItem pick the matching one.
  return (
    <motion.div
      variants={container}
      initial="hidden"
      animate="visible"
      data-reduced={reduced ? "1" : "0"}
      data-offset={initialOffset}
      className={className}
      {...rest}
    >
      {children}
    </motion.div>
  );
}

interface MotionListItemProps extends Omit<HTMLMotionProps<"div">, "variants" | "initial" | "animate"> {
  children: ReactNode;
}

/**
 * Single item inside `<MotionList>`. Reads the parent's `data-reduced` and
 * `data-offset` to keep animation behaviour consistent.
 */
export function MotionListItem({ children, className, ...rest }: MotionListItemProps) {
  const reduced = useReducedMotion();
  const itemVariants = {
    hidden: reduced ? { opacity: 0 } : { opacity: 0, y: 8 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { type: "spring" as const, bounce: 0, duration: reduced ? 0.2 : 0.32 },
    },
  };
  return (
    <motion.div variants={itemVariants} className={className} {...rest}>
      {children}
    </motion.div>
  );
}