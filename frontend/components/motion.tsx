"use client";

import type { ReactNode } from "react";
import { motion, type Variants } from "framer-motion";

const STAGGER_STEP = 0.055;
const ENTER_DURATION = 0.35;
const ENTER_OFFSET_PX = 8;

const staggerContainer: Variants = {
  hidden: {},
  visible: { transition: { staggerChildren: STAGGER_STEP, delayChildren: 0.02 } },
};

const staggerItem: Variants = {
  hidden: { opacity: 0, y: ENTER_OFFSET_PX },
  visible: {
    opacity: 1,
    y: 0,
    transition: { duration: ENTER_DURATION, ease: [0.16, 1, 0.3, 1] },
  },
};

export function MotionStagger({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <motion.div
      initial="hidden"
      animate="visible"
      variants={staggerContainer}
      className={className}
    >
      {children}
    </motion.div>
  );
}

export function MotionItem({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <motion.div variants={staggerItem} className={className}>
      {children}
    </motion.div>
  );
}
