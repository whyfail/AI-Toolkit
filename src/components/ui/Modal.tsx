import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useReducedMotion } from "@/hooks/useReducedMotion";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  /** Anchor element — modal scales from this rect. Used for popovers; pass `null` for centered. */
  anchor?: HTMLElement | null;
  /** Optional max-width Tailwind class. Default `max-w-lg`. */
  size?: "sm" | "md" | "lg" | "xl" | "2xl" | "full";
  /** z-index override. Default 50. */
  zIndex?: number;
  /** Hide the dimming backdrop (use for non-blocking panels). */
  noBackdrop?: boolean;
  /** Click on the backdrop dismisses. Default true. */
  dismissOnBackdrop?: boolean;
}

const sizeClass: Record<NonNullable<ModalProps["size"]>, string> = {
  sm: "max-w-sm",
  md: "max-w-md",
  lg: "max-w-lg",
  xl: "max-w-xl",
  "2xl": "max-w-2xl",
  full: "max-w-[min(96vw,1100px)]",
};

export function Modal({
  open,
  onClose,
  children,
  anchor,
  size = "lg",
  zIndex = 50,
  noBackdrop,
  dismissOnBackdrop = true,
}: ModalProps) {
  const reduced = useReducedMotion();
  const [portalTarget, setPortalTarget] = useState<HTMLElement | null>(null);

  // Mount once on the client to a body-level portal so the modal escapes any
  // ancestor that paints a mask / clip / overflow (e.g. `.glass-content`'s
  // edge-fade mask-image would otherwise trim the modal).
  useEffect(() => {
    setPortalTarget(document.body);
  }, []);

  // Anchor transform-origin for popovers (spatial consistency, §7).
  const origin = anchor
    ? (() => {
        const rect = anchor.getBoundingClientRect();
        return `${rect.left + rect.width / 2}px ${rect.top + rect.height / 2}px`;
      })()
    : "50% 50%";

  // SSR-safe: render the (closed) AnimatePresence tree inline until portal is ready.
  const tree = (
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 flex items-center justify-center p-2 sm:p-4"
          style={{ zIndex }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: reduced ? 0.2 : 0.18, ease: [0.16, 1, 0.3, 1] }}
        >
          {/* Backdrop */}
          {!noBackdrop && (
            <motion.button
              type="button"
              aria-label="关闭"
              tabIndex={-1}
              onClick={() => (dismissOnBackdrop ? onClose() : undefined)}
              className="absolute inset-0 cursor-default bg-black/45 backdrop-blur-sm dark:bg-black/65"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: reduced ? 0.2 : 0.18, ease: [0.16, 1, 0.3, 1] }}
            />
          )}
          {/* Content */}
          <motion.div
            role="dialog"
            aria-modal="true"
            className={`glass-modal relative mx-auto w-full ${sizeClass[size]} overflow-hidden`}
            style={{ transformOrigin: origin }}
            initial={reduced ? { opacity: 0 } : { opacity: 0, scale: 0.96, y: 4 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={reduced ? { opacity: 0 } : { opacity: 0, scale: 0.98, y: 2 }}
            transition={{ type: "spring", bounce: 0, duration: reduced ? 0.2 : 0.28 }}
          >
            {children}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );

  if (!portalTarget) return tree;
  return createPortal(tree, portalTarget);
}