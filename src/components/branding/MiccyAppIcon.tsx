import React from "react";
import { useTranslation } from "react-i18next";
import miccyLogoUrl from "@/assets/miccy-logo.png";

interface MiccyAppIconProps {
  readonly width?: number | string;
  readonly height?: number | string;
  readonly size?: number | string;
  readonly className?: string;
}

export function MiccyAppIcon({
  width,
  height,
  size = 24,
  className = "",
}: MiccyAppIconProps): React.ReactElement {
  const { t } = useTranslation();
  const dimension = width ?? height ?? size;
  return (
    <img
      src={miccyLogoUrl}
      alt={t("brand.logoAlt")}
      width={dimension}
      height={dimension}
      className={`object-contain ${className}`.trim()}
    />
  );
}
