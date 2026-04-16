import type { ReactNode } from "react";

interface MiccyMarkProps {
  readonly children?: ReactNode;
}

/**
 * Wordmark span for the product name. Use inside i18next {@link Trans} as the
 * {@code miccy} mapped component so localized inflections stay in translation files.
 */
export function MiccyMark({ children }: MiccyMarkProps): React.ReactElement {
  return (
    <span className="font-miccy text-[1.12em] leading-tight tracking-wide inline-block align-middle">
      {children ?? "Miccy"}
    </span>
  );
}
