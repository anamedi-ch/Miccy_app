import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export interface DropdownOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface DropdownProps {
  options: DropdownOption[];
  className?: string;
  selectedValue: string | null;
  onSelect: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  onRefresh?: () => void;
}

export const Dropdown: React.FC<DropdownProps> = ({
  options,
  selectedValue,
  onSelect,
  className = "",
  placeholder = "Select an option...",
  disabled = false,
  onRefresh,
}) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const selectedOption = options.find(
    (option) => option.value === selectedValue,
  );

  const handleSelect = (value: string) => {
    onSelect(value);
    setIsOpen(false);
  };

  const handleToggle = () => {
    if (disabled) return;
    if (!isOpen && onRefresh) onRefresh();
    setIsOpen(!isOpen);
  };

  return (
    <div className={`relative min-w-0 max-w-full ${className}`} ref={dropdownRef}>
      <button
        type="button"
        className={`min-h-[40px] w-full min-w-0 max-w-full px-3 py-2 text-sm font-medium bg-background border rounded-lg text-start flex items-center justify-between gap-2 transition-all duration-200 ${
          disabled
            ? "opacity-50 cursor-not-allowed border-mid-gray/30"
            : isOpen
              ? "border-logo-primary ring-2 ring-logo-primary/20 shadow-sm cursor-pointer"
              : "border-mid-gray/30 hover:border-mid-gray/50 hover:bg-mid-gray/5 cursor-pointer focus:outline-none focus:ring-2 focus:ring-logo-primary/20 focus:border-logo-primary"
        }`}
        onClick={handleToggle}
        disabled={disabled}
      >
        <span className="truncate text-text">
          {selectedOption?.label || placeholder}
        </span>
        <svg
          className={`w-4 h-4 shrink-0 text-mid-gray transition-transform duration-200 ${isOpen ? "rotate-180" : ""}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M19 9l-7 7-7-7"
          />
        </svg>
      </button>
      {isOpen && !disabled && (
        <div className="absolute top-full left-0 right-0 mt-1.5 bg-background border border-mid-gray/20 rounded-lg shadow-[0_4px_20px_rgba(0,0,0,0.12)] z-50 max-h-60 overflow-y-auto py-1">
          {options.length === 0 ? (
            <div className="px-3 py-2.5 text-sm text-mid-gray">
              {t("common.noOptionsFound")}
            </div>
          ) : (
            options.map((option) => (
              <button
                key={option.value}
                type="button"
                className={`w-full px-3 py-2 text-sm text-start transition-colors duration-150 ${
                  selectedValue === option.value
                    ? "bg-logo-primary/15 text-logo-primary font-medium"
                    : "text-text hover:bg-mid-gray/10"
                } ${option.disabled ? "opacity-50 cursor-not-allowed" : ""}`}
                onClick={() => handleSelect(option.value)}
                disabled={option.disabled}
              >
                <span className="truncate block">{option.label}</span>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
};
