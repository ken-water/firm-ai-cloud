import type { CSSProperties } from "react";

type HorizontalFillBarProps = {
  width: string;
  color: string;
  height?: string;
};

export function HorizontalFillBar({ width, color, height = "100%" }: HorizontalFillBarProps) {
  const style: CSSProperties = {
    width,
    height,
    background: color
  };
  return <div style={style} />;
}

type MetricSparklineProps = {
  ariaLabel: string;
  points: string;
  stroke?: string;
};

export function MetricSparkline({ ariaLabel, points, stroke = "#1d4ed8" }: MetricSparklineProps) {
  return (
    <svg viewBox="0 0 320 120" style={{ width: "100%", height: "120px", display: "block" }} aria-label={ariaLabel}>
      <rect x="0" y="0" width="320" height="120" fill="#f8fafc" />
      <polyline
        fill="none"
        stroke={stroke}
        strokeWidth="2.6"
        strokeLinejoin="round"
        strokeLinecap="round"
        points={points}
      />
    </svg>
  );
}
