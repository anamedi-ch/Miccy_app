import React from "react";
import { useTranslation } from "react-i18next";
import miccyLogoUrl from "@/assets/miccy-logo.png";

interface MiccyHeroLogoProps {
  readonly width?: number;
  readonly className?: string;
}

export function MiccyHeroLogo({
  width = 200,
  className,
}: MiccyHeroLogoProps): React.ReactElement {
  const { t } = useTranslation();
  return (
    <img
      src={miccyLogoUrl}
      alt={t("brand.logoAlt")}
      width={width}
      className={`object-contain max-w-full h-auto ${className ?? ""}`.trim()}
      style={{ maxHeight: width * 1.2 }}
    />
  );
}
