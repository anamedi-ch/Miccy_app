import React from "react";

const HandyTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <svg
      width={width ?? 200}
      height={height ?? 40}
      className={className}
      viewBox="0 0 200 40"
      xmlns="http://www.w3.org/2000/svg"
    >
      <text
        x="0"
        y="50%"
        dominantBaseline="middle"
        fill="var(--color-logo-primary)"
        fontFamily="system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
        fontSize="22"
        fontWeight="700"
        letterSpacing="1"
      >
        anamedi local
      </text>
    </svg>
  );
};

export default HandyTextLogo;
