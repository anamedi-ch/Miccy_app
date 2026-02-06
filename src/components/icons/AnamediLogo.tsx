import React from "react";

import anamediIcon from "@/assets/icons/anamedi-icon.png";

interface AnamediLogoProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
}

const AnamediLogo: React.FC<AnamediLogoProps> = ({
  width,
  height,
  size = 24,
  className = "",
}) => {
  const dimension = width ?? height ?? size;

  return (
    <img
      src={anamediIcon}
      alt="Anamedi"
      width={dimension}
      height={dimension}
      className={`object-contain ${className}`.trim()}
    />
  );
};

export default AnamediLogo;
