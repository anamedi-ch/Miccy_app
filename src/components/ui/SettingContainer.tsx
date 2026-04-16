import React, { useEffect, useRef, useState } from "react";
import { Tooltip } from "./Tooltip";

interface SettingContainerProps {
  title: React.ReactNode;
  description: React.ReactNode;
  children: React.ReactNode;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  layout?: "horizontal" | "stacked";
  disabled?: boolean;
  tooltipPosition?: "top" | "bottom";
}

export const SettingContainer: React.FC<SettingContainerProps> = ({
  title,
  description,
  children,
  descriptionMode = "tooltip",
  grouped = false,
  layout = "horizontal",
  disabled = false,
  tooltipPosition = "top",
}) => {
  const [showTooltip, setShowTooltip] = useState(false);
  const tooltipRef = useRef<HTMLDivElement>(null);

  // Handle click outside to close tooltip
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        tooltipRef.current &&
        !tooltipRef.current.contains(event.target as Node)
      ) {
        setShowTooltip(false);
      }
    };

    if (showTooltip) {
      document.addEventListener("mousedown", handleClickOutside);
      return () =>
        document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [showTooltip]);

  const toggleTooltip = () => {
    setShowTooltip(!showTooltip);
  };

  const containerClasses = grouped
    ? "px-4 p-2"
    : "px-4 p-2 rounded-lg border border-mid-gray/20";

  if (layout === "stacked") {
    if (descriptionMode === "tooltip") {
      return (
        <div className={`${containerClasses} min-w-0`}>
          <div className="flex items-center gap-2 mb-2 min-w-0">
            <h3
              className={`text-sm font-medium min-w-0 flex-1 truncate ${disabled ? "opacity-50" : ""}`}
            >
              {title}
            </h3>
            <div
              ref={tooltipRef}
              className="relative shrink-0"
              onMouseEnter={() => setShowTooltip(true)}
              onMouseLeave={() => setShowTooltip(false)}
              onClick={toggleTooltip}
            >
              <svg
                className="w-4 h-4 text-mid-gray cursor-help hover:text-logo-primary transition-colors duration-200 select-none"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                aria-label="More information"
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    toggleTooltip();
                  }
                }}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              {showTooltip && (
                <Tooltip targetRef={tooltipRef} position="top">
                  <p className="text-sm text-center leading-relaxed">
                    {description}
                  </p>
                </Tooltip>
              )}
            </div>
          </div>
          <div className="w-full min-w-0 max-w-full">{children}</div>
        </div>
      );
    }

    return (
      <div className={`${containerClasses} min-w-0`}>
        <div className="mb-2">
          <h3 className={`text-sm font-medium ${disabled ? "opacity-50" : ""}`}>
            {title}
          </h3>
          <p className={`text-sm ${disabled ? "opacity-50" : ""}`}>
            {description}
          </p>
        </div>
        <div className="w-full min-w-0 max-w-full">{children}</div>
      </div>
    );
  }

  // Horizontal layout (default): min-w-0 + flex-1 on the control side prevents
  // dropdowns and long labels from overflowing the settings card.
  const horizontalContainerClasses = grouped
    ? "flex items-center gap-3 px-4 p-2 min-w-0"
    : "flex items-center gap-3 px-4 p-2 rounded-lg border border-mid-gray/20 min-w-0";

  if (descriptionMode === "tooltip") {
    return (
      <div className={horizontalContainerClasses}>
        <div className="min-w-0 shrink-0 max-w-[50%] pe-1">
          <div className="flex items-center gap-2 min-w-0">
            <h3
              className={`text-sm font-medium min-w-0 flex-1 truncate ${disabled ? "opacity-50" : ""}`}
            >
              {title}
            </h3>
            <div
              ref={tooltipRef}
              className="relative shrink-0"
              onMouseEnter={() => setShowTooltip(true)}
              onMouseLeave={() => setShowTooltip(false)}
              onClick={toggleTooltip}
            >
              <svg
                className="w-4 h-4 text-mid-gray cursor-help hover:text-logo-primary transition-colors duration-200 select-none"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                aria-label="More information"
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    toggleTooltip();
                  }
                }}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              {showTooltip && (
                <Tooltip targetRef={tooltipRef} position={tooltipPosition}>
                  <p className="text-sm text-center leading-relaxed">
                    {description}
                  </p>
                </Tooltip>
              )}
            </div>
          </div>
        </div>
        <div className="relative min-w-0 flex-1 flex justify-end items-center ps-1">
          <div className="min-w-0 w-full max-w-full">{children}</div>
        </div>
      </div>
    );
  }

  return (
    <div className={horizontalContainerClasses}>
      <div className="min-w-0 shrink-0 max-w-[50%] pe-1">
        <h3
          className={`text-sm font-medium truncate ${disabled ? "opacity-50" : ""}`}
        >
          {title}
        </h3>
        <p
          className={`text-sm line-clamp-3 break-words ${disabled ? "opacity-50" : ""}`}
        >
          {description}
        </p>
      </div>
      <div className="relative min-w-0 flex-1 flex justify-end items-center ps-1">
        <div className="min-w-0 w-full max-w-full">{children}</div>
      </div>
    </div>
  );
};
